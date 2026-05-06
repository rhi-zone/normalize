//! Embedding population: walk symbols from the structural index and embed them.
//!
//! Called after `structure rebuild` when `embeddings.enabled = true`.
//! For incremental rebuilds, only symbols in changed files are re-embedded.
//!
//! Additional source types:
//! - [`populate_markdown_docs`] -- embed `*.md` files under `docs/`, `SUMMARY.md`,
//!   `CLAUDE.md`, and `README.md` chunked by heading section.
//! - [`populate_commit_messages`] -- embed recent git commit messages (last N commits)
//!   keyed by commit hash.
//! - [`populate_incremental_for_paths`] -- incremental re-embed for a set of changed
//!   paths; called by the daemon on file change without a full rebuild.

use crate::chunks::{
    SymbolRow, build_commit_chunk, build_markdown_chunk, build_symbol_chunk,
    split_markdown_sections,
};
use crate::config::EmbeddingsConfig;
use crate::embedder::{Embedder, encode_vector};
use crate::git_staleness::compute_staleness_batch;
use crate::store;
use crate::vec_ext::VecConnection;
use gix::bstr::ByteSlice as _;
use libsql::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tracing::{info, warn};

/// Result of a population run.
#[derive(Debug, Default)]
pub struct PopulateStats {
    pub symbols_embedded: usize,
    pub symbols_skipped: usize,
    pub docs_embedded: usize,
    pub commits_embedded: usize,
    pub errors: usize,
}

/// Batch size for embedding calls (fastembed handles batching internally but
/// we chunk to keep peak memory bounded).
const EMBED_BATCH_SIZE: usize = 64;

/// Default number of recent commits to embed.
pub const DEFAULT_MAX_COMMITS: usize = 500;

/// Populate embeddings for all symbols in the index.
///
/// If `incremental` is true and `changed_paths` is provided, only symbols from
/// those files are re-embedded. Otherwise all symbols are re-embedded.
///
/// `db_path` is the path to the SQLite database file, used to open a parallel
/// raw connection with sqlite-vec registered for ANN virtual table operations.
pub async fn populate_embeddings(
    conn: &Connection,
    config: &EmbeddingsConfig,
    changed_paths: Option<&[String]>,
    head_commit: Option<&str>,
    repo_root: Option<&std::path::Path>,
    db_path: Option<&Path>,
) -> anyhow::Result<PopulateStats> {
    let started = Instant::now();
    let is_full_rebuild = changed_paths.is_none();

    // Open a parallel raw connection with sqlite-vec registered for ANN
    // virtual table operations (CREATE VIRTUAL TABLE, INSERT, DELETE).
    let vec_conn: Option<VecConnection> = db_path.and_then(VecConnection::open);

    // For a full rebuild, drop and recreate tables instead of per-row deletes.
    // This avoids leaving massive dead pages in SQLite.
    if is_full_rebuild {
        store::drop_embedding_tables(conn, vec_conn.as_ref()).await?;
    }

    store::ensure_schema(conn).await?;

    eprintln!("Loading embedding model {}...", config.model);
    let mut embedder = Embedder::load(&config.model, None)?;
    info!(model = %config.model, dims = embedder.dimensions, "Embedding model loaded");
    eprintln!(
        "Loaded model {} ({} dimensions)",
        config.model, embedder.dimensions
    );

    // Create the ANN virtual table now that we know the model's dimension count.
    store::ensure_vec_schema(conn, embedder.dimensions, vec_conn.as_ref()).await;

    let mut stats = PopulateStats::default();

    // Load all symbols
    let symbols = load_symbols(conn, changed_paths).await?;

    if symbols.is_empty() {
        eprintln!("No symbols to embed.");
        return Ok(stats);
    }

    let total = symbols.len();
    eprintln!("Embedding {total} symbols...");

    // If incremental, delete old embeddings for changed paths first
    if let Some(paths) = changed_paths {
        for path in paths {
            store::delete_embeddings_for_path(conn, path, vec_conn.as_ref()).await?;
        }
    }

    // Build context maps with single bulk queries instead of per-symbol queries.
    let co_change_map = load_co_change_map(conn).await?;
    eprintln!("Loading callers...");
    let all_callers = load_all_callers(conn).await;
    eprintln!("Loading callees...");
    let all_callees = load_all_callees(conn).await;
    eprintln!("Loading doc comments...");
    let all_docs = load_all_doc_comments(conn).await;

    // Compute staleness per unique file path
    let file_paths: Vec<&str> = symbols.iter().map(|s| s.file.as_str()).collect();
    let staleness_map: HashMap<String, f64> = if let Some(root) = repo_root {
        compute_staleness_batch(root, &file_paths)
    } else {
        HashMap::new()
    };

    // Process in batches
    let mut batch_symbols: Vec<SymbolRow> = Vec::new();
    let mut batch_texts: Vec<String> = Vec::new();
    let mut batch_staleness: Vec<f64> = Vec::new();
    let mut done = 0usize;

    for symbol in symbols {
        let callers = all_callers.get(&symbol.name).cloned().unwrap_or_default();
        let callees = all_callees
            .get(&(symbol.name.clone(), symbol.file.clone()))
            .cloned()
            .unwrap_or_default();
        let co_changes = co_change_map.get(&symbol.file).cloned().unwrap_or_default();
        let doc = all_docs
            .get(&(symbol.name.clone(), symbol.file.clone()))
            .cloned();
        let chunk_text =
            build_symbol_chunk(&symbol, doc.as_deref(), &callers, &callees, &co_changes);
        let staleness = *staleness_map.get(&symbol.file).unwrap_or(&0.0);

        batch_symbols.push(symbol);
        batch_texts.push(chunk_text);
        batch_staleness.push(staleness);

        if batch_texts.len() >= EMBED_BATCH_SIZE {
            flush_batch(
                conn,
                &mut embedder,
                &batch_symbols,
                &batch_texts,
                &batch_staleness,
                head_commit,
                &config.model,
                &mut stats,
                vec_conn.as_ref(),
            )
            .await;
            done += batch_symbols.len();
            eprintln!("Embedded {done}/{total} symbols");
            batch_symbols.clear();
            batch_texts.clear();
            batch_staleness.clear();
        }
    }

    if !batch_texts.is_empty() {
        flush_batch(
            conn,
            &mut embedder,
            &batch_symbols,
            &batch_texts,
            &batch_staleness,
            head_commit,
            &config.model,
            &mut stats,
            vec_conn.as_ref(),
        )
        .await;
        done += batch_symbols.len();
        eprintln!("Embedded {done}/{total} symbols");
    }

    if is_full_rebuild {
        eprintln!("Running VACUUM to reclaim space...");
        store::vacuum(conn).await;
    }

    let elapsed = started.elapsed().as_secs_f64();
    eprintln!("Embedding complete. {total} symbols in {elapsed:.1}s");
    info!(
        embedded = stats.symbols_embedded,
        errors = stats.errors,
        elapsed_secs = elapsed,
        "Embedding population complete"
    );

    Ok(stats)
}

/// Incremental re-embedding for a set of changed file paths.
///
/// Called by the daemon after `incremental_refresh` when `embeddings.enabled` is true.
/// Deletes old embeddings for each changed path, then re-embeds symbols and markdown
/// docs in those files. Lightweight -- no VACUUM, no full table drop.
pub async fn populate_incremental_for_paths(
    conn: &Connection,
    config: &EmbeddingsConfig,
    changed_paths: &[String],
    head_commit: Option<&str>,
    repo_root: Option<&Path>,
    db_path: Option<&Path>,
) -> anyhow::Result<PopulateStats> {
    if changed_paths.is_empty() {
        return Ok(PopulateStats::default());
    }

    let vec_conn: Option<VecConnection> = db_path.and_then(VecConnection::open);
    store::ensure_schema(conn).await?;

    let mut embedder = Embedder::load(&config.model, None)?;
    store::ensure_vec_schema(conn, embedder.dimensions, vec_conn.as_ref()).await;

    let mut stats = PopulateStats::default();

    // Delete old embeddings for changed paths (all source types)
    for path in changed_paths {
        if let Err(e) = store::delete_embeddings_for_path(conn, path, vec_conn.as_ref()).await {
            warn!(path, error = %e, "Failed to delete old embeddings for changed path");
        }
    }

    // Re-embed symbols in changed source files
    let symbols = load_symbols(conn, Some(changed_paths)).await?;
    if !symbols.is_empty() {
        let co_change_map = load_co_change_map(conn).await?;
        let all_callers = load_all_callers(conn).await;
        let all_callees = load_all_callees(conn).await;
        let all_docs = load_all_doc_comments(conn).await;

        let file_paths: Vec<&str> = symbols.iter().map(|s| s.file.as_str()).collect();
        let staleness_map: HashMap<String, f64> = if let Some(root) = repo_root {
            compute_staleness_batch(root, &file_paths)
        } else {
            HashMap::new()
        };

        let mut batch_symbols: Vec<SymbolRow> = Vec::new();
        let mut batch_texts: Vec<String> = Vec::new();
        let mut batch_staleness: Vec<f64> = Vec::new();

        for symbol in symbols {
            let callers = all_callers.get(&symbol.name).cloned().unwrap_or_default();
            let callees = all_callees
                .get(&(symbol.name.clone(), symbol.file.clone()))
                .cloned()
                .unwrap_or_default();
            let co_changes = co_change_map.get(&symbol.file).cloned().unwrap_or_default();
            let doc = all_docs
                .get(&(symbol.name.clone(), symbol.file.clone()))
                .cloned();
            let chunk_text =
                build_symbol_chunk(&symbol, doc.as_deref(), &callers, &callees, &co_changes);
            let staleness = *staleness_map.get(&symbol.file).unwrap_or(&0.0);

            batch_symbols.push(symbol);
            batch_texts.push(chunk_text);
            batch_staleness.push(staleness);

            if batch_texts.len() >= EMBED_BATCH_SIZE {
                flush_batch(
                    conn,
                    &mut embedder,
                    &batch_symbols,
                    &batch_texts,
                    &batch_staleness,
                    head_commit,
                    &config.model,
                    &mut stats,
                    vec_conn.as_ref(),
                )
                .await;
                batch_symbols.clear();
                batch_texts.clear();
                batch_staleness.clear();
            }
        }
        if !batch_texts.is_empty() {
            flush_batch(
                conn,
                &mut embedder,
                &batch_symbols,
                &batch_texts,
                &batch_staleness,
                head_commit,
                &config.model,
                &mut stats,
                vec_conn.as_ref(),
            )
            .await;
        }
    }

    // Re-embed any changed markdown docs
    if let Some(root) = repo_root {
        for path in changed_paths {
            if path.ends_with(".md") {
                let abs_path = root.join(path);
                if let Ok(content) = std::fs::read_to_string(&abs_path) {
                    let sections = split_markdown_sections(&content);
                    let mut md_texts: Vec<String> = Vec::new();
                    let mut md_ids: Vec<i64> = Vec::new();
                    for (i, (breadcrumb, body)) in sections.iter().enumerate() {
                        if body.trim().is_empty() {
                            continue;
                        }
                        md_texts.push(build_markdown_chunk(path, breadcrumb, body));
                        md_ids.push(i as i64);
                    }
                    if !md_texts.is_empty() {
                        flush_doc_batch(
                            conn,
                            &mut embedder,
                            path,
                            &md_texts,
                            &md_ids,
                            head_commit,
                            &config.model,
                            &mut stats,
                            vec_conn.as_ref(),
                        )
                        .await;
                    }
                }
            }
        }
    }

    info!(
        symbols = stats.symbols_embedded,
        docs = stats.docs_embedded,
        paths = changed_paths.len(),
        "Incremental re-embedding complete"
    );

    Ok(stats)
}

/// Embed all eligible markdown files in the workspace.
///
/// Eligible files:
/// - `SUMMARY.md`, `CLAUDE.md`, `README.md` in the repo root
/// - Any `*.md` under `docs/`
///
/// Each file is chunked by heading sections. Existing `doc` embeddings for the
/// same paths are replaced via the UNIQUE constraint on `(source_type, source_path, source_id)`.
pub async fn populate_markdown_docs(
    conn: &Connection,
    config: &EmbeddingsConfig,
    repo_root: &Path,
    head_commit: Option<&str>,
    db_path: Option<&Path>,
) -> anyhow::Result<PopulateStats> {
    let vec_conn: Option<VecConnection> = db_path.and_then(VecConnection::open);
    store::ensure_schema(conn).await?;

    let mut embedder = Embedder::load(&config.model, None)?;
    store::ensure_vec_schema(conn, embedder.dimensions, vec_conn.as_ref()).await;

    let mut stats = PopulateStats::default();

    let mut md_files: Vec<std::path::PathBuf> = Vec::new();

    for name in &["SUMMARY.md", "CLAUDE.md", "README.md"] {
        let p = repo_root.join(name);
        if p.exists() {
            md_files.push(p);
        }
    }

    let docs_dir = repo_root.join("docs");
    if docs_dir.is_dir() {
        collect_md_files(&docs_dir, &mut md_files);
    }

    if md_files.is_empty() {
        return Ok(stats);
    }

    eprintln!("Embedding {} markdown document(s)...", md_files.len());

    for abs_path in &md_files {
        let rel_path = abs_path
            .strip_prefix(repo_root)
            .unwrap_or(abs_path)
            .to_string_lossy()
            .into_owned();

        if let Err(e) = store::delete_embeddings_for_path(conn, &rel_path, vec_conn.as_ref()).await
        {
            warn!(path = %rel_path, error = %e, "Failed to delete old doc embeddings");
        }

        let content = match std::fs::read_to_string(abs_path) {
            Ok(c) => c,
            Err(e) => {
                warn!(path = %rel_path, error = %e, "Could not read markdown file");
                stats.errors += 1;
                continue;
            }
        };

        let sections = split_markdown_sections(&content);
        let mut texts: Vec<String> = Vec::new();
        let mut section_ids: Vec<i64> = Vec::new();

        for (i, (breadcrumb, body)) in sections.iter().enumerate() {
            if body.trim().is_empty() {
                continue;
            }
            texts.push(build_markdown_chunk(&rel_path, breadcrumb, body));
            section_ids.push(i as i64);
        }

        if texts.is_empty() {
            continue;
        }

        flush_doc_batch(
            conn,
            &mut embedder,
            &rel_path,
            &texts,
            &section_ids,
            head_commit,
            &config.model,
            &mut stats,
            vec_conn.as_ref(),
        )
        .await;
    }

    info!(
        docs = stats.docs_embedded,
        files = md_files.len(),
        "Markdown doc embedding complete"
    );

    Ok(stats)
}

/// Recursively collect `*.md` files under a directory (sorted for determinism).
fn collect_md_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_md_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

/// Embed recent git commit messages.
///
/// Walks up to `max_commits` commits from HEAD (using `gix`) and embeds each
/// commit's subject + body as a `commit` source chunk, keyed by the short hash.
///
/// Commits already present in the embeddings table (same hash) are skipped so
/// repeated runs only embed new commits.
pub async fn populate_commit_messages(
    conn: &Connection,
    config: &EmbeddingsConfig,
    repo_root: &Path,
    head_commit: Option<&str>,
    db_path: Option<&Path>,
    max_commits: usize,
) -> anyhow::Result<PopulateStats> {
    let vec_conn: Option<VecConnection> = db_path.and_then(VecConnection::open);
    store::ensure_schema(conn).await?;

    let mut embedder = Embedder::load(&config.model, None)?;
    store::ensure_vec_schema(conn, embedder.dimensions, vec_conn.as_ref()).await;

    let mut stats = PopulateStats::default();

    let embedded_hashes = load_embedded_commit_hashes(conn, &config.model).await;

    let commits = load_recent_commits(repo_root, max_commits);
    if commits.is_empty() {
        return Ok(stats);
    }

    let new_commits: Vec<CommitInfo> = commits
        .into_iter()
        .filter(|c| !embedded_hashes.contains(c.hash.as_str()))
        .collect();

    if new_commits.is_empty() {
        return Ok(stats);
    }

    eprintln!("Embedding {} new commit message(s)...", new_commits.len());

    let mut texts: Vec<String> = Vec::new();
    let mut hashes: Vec<String> = Vec::new();

    for commit in &new_commits {
        let chunk = build_commit_chunk(&commit.hash, &commit.date, &commit.subject, &commit.body);
        texts.push(chunk);
        hashes.push(commit.hash.clone());
    }

    for (chunk_texts, chunk_hashes) in texts
        .chunks(EMBED_BATCH_SIZE)
        .zip(hashes.chunks(EMBED_BATCH_SIZE))
    {
        let text_refs: Vec<&str> = chunk_texts.iter().map(String::as_str).collect();
        match embedder.embed_batch(&text_refs) {
            Ok(vectors) => {
                if let Err(e) = conn.execute("BEGIN", ()).await {
                    warn!(error = %e, "Failed to BEGIN transaction for commit batch");
                }
                for (i, (text, vec)) in chunk_texts.iter().zip(vectors.iter()).enumerate() {
                    let hash = &chunk_hashes[i];
                    let blob = encode_vector(vec);
                    match store::upsert_embedding(
                        conn,
                        "commit",
                        hash,
                        None,
                        &config.model,
                        head_commit,
                        0.0,
                        text,
                        &blob,
                        vec_conn.as_ref(),
                    )
                    .await
                    {
                        Ok(()) => stats.commits_embedded += 1,
                        Err(e) => {
                            warn!(hash, error = %e, "Failed to store commit embedding");
                            stats.errors += 1;
                        }
                    }
                }
                if let Err(e) = conn.execute("COMMIT", ()).await {
                    warn!(error = %e, "Failed to COMMIT transaction for commit batch");
                }
            }
            Err(e) => {
                warn!(error = %e, "Commit embedding batch failed");
                stats.errors += chunk_texts.len();
            }
        }
    }

    info!(
        commits = stats.commits_embedded,
        "Commit message embedding complete"
    );

    Ok(stats)
}

/// A parsed commit entry from the git history.
struct CommitInfo {
    hash: String,
    date: String,
    subject: String,
    body: String,
}

/// Walk up to `max_commits` commits from HEAD using `gix` and return their info.
fn load_recent_commits(root: &Path, max_commits: usize) -> Vec<CommitInfo> {
    let repo = match gix::discover(root) {
        Ok(r) => r.into_sync().to_thread_local(),
        Err(_) => return Vec::new(),
    };

    let head_id = match repo.head_id() {
        Ok(id) => id,
        Err(_) => return Vec::new(),
    };

    let walk = match head_id
        .ancestors()
        .sorting(gix::revision::walk::Sorting::ByCommitTime(
            gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
        ))
        .all()
    {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };

    let mut commits = Vec::new();

    for info in walk.take(max_commits) {
        let Ok(info) = info else { continue };
        let Ok(commit_obj) = info.object() else {
            continue;
        };
        let Ok(commit) = commit_obj.decode() else {
            continue;
        };

        let hash = info.id().to_string();
        let short_hash = if hash.len() >= 12 {
            hash[..12].to_string()
        } else {
            hash.clone()
        };

        // commit.time() returns Result<Time, Error>; fall back to 0 on decode error.
        let timestamp = commit.time().map(|t| t.seconds).unwrap_or(0);
        let date = epoch_to_date(timestamp);

        // commit.message is &BStr; ByteSlice::to_str_lossy for UTF-8 conversion.
        let full_message = commit.message.to_str_lossy().into_owned();
        let msg_ref = commit.message();
        let subject = msg_ref.summary().to_str_lossy().trim().to_string();
        let body = full_message
            .trim_start_matches(subject.as_str())
            .trim()
            .to_string();

        if subject.is_empty() {
            continue;
        }

        commits.push(CommitInfo {
            hash: short_hash,
            date,
            subject,
            body,
        });
    }

    commits
}

/// Convert Unix epoch seconds to a `YYYY-MM-DD` string (UTC, approximate).
///
/// Uses pure arithmetic -- no chrono dependency.
fn epoch_to_date(secs: i64) -> String {
    let days_since_epoch = secs.max(0) as u64 / 86400;

    // Civil calendar from Howard Hinnant's date algorithms
    let z = days_since_epoch as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}")
}

/// Load the set of commit hashes already present in the embeddings table.
async fn load_embedded_commit_hashes(
    conn: &Connection,
    model: &str,
) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let Ok(mut rows) = conn
        .query(
            "SELECT source_path FROM embeddings WHERE source_type = 'commit' AND model = ?1",
            [model],
        )
        .await
    else {
        return set;
    };
    while let Ok(Some(row)) = rows.next().await {
        if let Ok(hash) = row.get::<String>(0) {
            set.insert(hash);
        }
    }
    set
}

/// Flush a batch of doc (markdown) chunks through the embedder and store.
///
/// `section_ids` are used as `source_id` so each heading section gets its own
/// unique slot under the UNIQUE constraint `(source_type='doc', source_path, source_id=i)`.
#[allow(clippy::too_many_arguments)]
async fn flush_doc_batch(
    conn: &Connection,
    embedder: &mut Embedder,
    rel_path: &str,
    texts: &[String],
    section_ids: &[i64],
    head_commit: Option<&str>,
    model_name: &str,
    stats: &mut PopulateStats,
    vec_conn: Option<&VecConnection>,
) {
    let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    match embedder.embed_batch(&text_refs) {
        Ok(vectors) => {
            if let Err(e) = conn.execute("BEGIN", ()).await {
                warn!(error = %e, "Failed to BEGIN transaction for doc batch");
            }
            for (i, (text, vec)) in texts.iter().zip(vectors.iter()).enumerate() {
                let blob = encode_vector(vec);
                let sid = section_ids.get(i).copied().unwrap_or(i as i64);
                match store::upsert_embedding(
                    conn,
                    "doc",
                    rel_path,
                    Some(sid),
                    model_name,
                    head_commit,
                    0.0,
                    text,
                    &blob,
                    vec_conn,
                )
                .await
                {
                    Ok(()) => stats.docs_embedded += 1,
                    Err(e) => {
                        warn!(path = %rel_path, section = i, error = %e, "Failed to store doc embedding");
                        stats.errors += 1;
                    }
                }
            }
            if let Err(e) = conn.execute("COMMIT", ()).await {
                warn!(error = %e, "Failed to COMMIT transaction for doc batch");
            }
        }
        Err(e) => {
            warn!(error = %e, path = %rel_path, "Doc embedding batch failed");
            stats.errors += texts.len();
        }
    }
}

/// Flush one batch of symbols through the embedder and store.
#[allow(clippy::too_many_arguments)]
async fn flush_batch(
    conn: &Connection,
    embedder: &mut Embedder,
    symbols: &[SymbolRow],
    texts: &[String],
    staleness: &[f64],
    head_commit: Option<&str>,
    model_name: &str,
    stats: &mut PopulateStats,
    vec_conn: Option<&VecConnection>,
) {
    let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    match embedder.embed_batch(&text_refs) {
        Ok(vectors) => {
            if let Err(e) = conn.execute("BEGIN", ()).await {
                warn!(error = %e, "Failed to BEGIN transaction for batch");
            }
            for (idx, (sym, (text, vec))) in symbols
                .iter()
                .zip(texts.iter().zip(vectors.iter()))
                .enumerate()
            {
                let blob = encode_vector(vec);
                let sym_staleness = staleness.get(idx).copied().unwrap_or(0.0) as f32;
                match store::upsert_embedding(
                    conn,
                    "symbol",
                    &sym.file,
                    Some(sym.rowid),
                    model_name,
                    head_commit,
                    sym_staleness,
                    text,
                    &blob,
                    vec_conn,
                )
                .await
                {
                    Ok(()) => stats.symbols_embedded += 1,
                    Err(e) => {
                        warn!(symbol = %sym.name, file = %sym.file, error = %e, "Failed to store embedding");
                        stats.errors += 1;
                    }
                }
            }
            if let Err(e) = conn.execute("COMMIT", ()).await {
                warn!(error = %e, "Failed to COMMIT transaction for batch");
            }
        }
        Err(e) => {
            warn!(error = %e, batch_size = symbols.len(), "Embedding batch failed");
            stats.errors += symbols.len();
        }
    }
}

/// Load all symbols from the index (or only those in changed_paths).
async fn load_symbols(
    conn: &Connection,
    changed_paths: Option<&[String]>,
) -> anyhow::Result<Vec<SymbolRow>> {
    let sql = "SELECT rowid, file, name, kind, start_line, end_line, parent FROM symbols";

    let mut rows = conn.query(sql, ()).await?;
    let mut symbols = Vec::new();

    let path_set: Option<std::collections::HashSet<&str>> =
        changed_paths.map(|paths| paths.iter().map(String::as_str).collect());

    while let Some(row) = rows.next().await? {
        let file: String = row.get(1)?;

        if path_set
            .as_ref()
            .is_some_and(|set| !set.contains(file.as_str()))
        {
            continue;
        }

        symbols.push(SymbolRow {
            rowid: row.get(0)?,
            file,
            name: row.get(2)?,
            kind: row.get(3)?,
            start_line: row.get(4)?,
            end_line: row.get(5)?,
            parent: row.get(6)?,
        });
    }

    Ok(symbols)
}

/// Load all callers in one query, grouped by callee name.
/// Returns a map: callee_name -> top-10 caller names (by call count).
async fn load_all_callers(conn: &Connection) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    let Ok(mut rows) = conn
        .query(
            "SELECT callee_name, caller_symbol, COUNT(*) as cnt \
             FROM calls \
             GROUP BY callee_name, caller_symbol \
             ORDER BY callee_name, cnt DESC",
            (),
        )
        .await
    else {
        return map;
    };

    while let Ok(Some(row)) = rows.next().await {
        let Ok(callee) = row.get::<String>(0) else {
            continue;
        };
        let Ok(caller) = row.get::<String>(1) else {
            continue;
        };
        let entry = map.entry(callee).or_default();
        if entry.len() < 10 {
            entry.push(caller);
        }
    }

    map
}

/// Load all callees in one query, grouped by (caller_symbol, caller_file).
/// Returns a map: (caller_symbol, caller_file) -> callee names (up to 10).
async fn load_all_callees(conn: &Connection) -> HashMap<(String, String), Vec<String>> {
    let mut map: HashMap<(String, String), Vec<String>> = HashMap::new();

    let Ok(mut rows) = conn
        .query(
            "SELECT caller_symbol, caller_file, callee_name FROM calls ORDER BY caller_symbol, caller_file",
            (),
        )
        .await
    else {
        return map;
    };

    while let Ok(Some(row)) = rows.next().await {
        let Ok(caller_sym) = row.get::<String>(0) else {
            continue;
        };
        let Ok(caller_file) = row.get::<String>(1) else {
            continue;
        };
        let Ok(callee) = row.get::<String>(2) else {
            continue;
        };
        let entry = map.entry((caller_sym, caller_file)).or_default();
        if entry.len() < 10 {
            entry.push(callee);
        }
    }

    map
}

/// Load all doc comments in one query from the symbol_attributes table.
/// Returns a map: (name, file) -> doc text.
async fn load_all_doc_comments(conn: &Connection) -> HashMap<(String, String), String> {
    let mut map: HashMap<(String, String), String> = HashMap::new();

    let Ok(mut rows) = conn
        .query(
            "SELECT name, file, attribute FROM symbol_attributes WHERE attribute LIKE 'doc:%' ORDER BY name, file",
            (),
        )
        .await
    else {
        return map;
    };

    while let Ok(Some(row)) = rows.next().await {
        let Ok(name) = row.get::<String>(0) else {
            continue;
        };
        let Ok(file) = row.get::<String>(1) else {
            continue;
        };
        let Ok(attr) = row.get::<String>(2) else {
            continue;
        };
        if let Some(doc) = attr.strip_prefix("doc:") {
            let entry = map.entry((name, file)).or_default();
            if !entry.is_empty() {
                entry.push('\n');
            }
            entry.push_str(doc);
        }
    }

    map
}

/// Load co-change neighbors for all files as a map: file -> [neighbor_files].
/// Returns an empty map if the co_change_edges table doesn't exist yet.
async fn load_co_change_map(
    conn: &Connection,
) -> anyhow::Result<std::collections::HashMap<String, Vec<String>>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    let mut rows = match conn
        .query(
            "SELECT file_a, file_b FROM co_change_edges ORDER BY count DESC",
            (),
        )
        .await
    {
        Ok(rows) => rows,
        Err(_) => {
            return Ok(map);
        }
    };

    while let Some(row) = rows.next().await? {
        let a: String = row.get(0)?;
        let b: String = row.get(1)?;
        map.entry(a.clone()).or_default().push(b.clone());
        map.entry(b).or_default().push(a);
    }

    Ok(map)
}
