//! Embedding population: walk symbols from the structural index and embed them.
//!
//! Called after `structure rebuild` when `embeddings.enabled = true`.
//! For incremental rebuilds, only symbols in changed files are re-embedded.

use crate::chunks::{SymbolRow, build_symbol_chunk};
use crate::config::EmbeddingsConfig;
use crate::embedder::{Embedder, encode_vector};
use crate::git_staleness::compute_staleness_batch;
use crate::store;
use crate::vec_ext::VecConnection;
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
    pub errors: usize,
}

/// Batch size for embedding calls (fastembed handles batching internally but
/// we chunk to keep peak memory bounded).
const EMBED_BATCH_SIZE: usize = 64;

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

    // Build context for co-change lookups (file → co-change neighbors)
    let co_change_map = load_co_change_map(conn).await?;

    // Compute staleness per unique file path — one git walk per file, not per symbol.
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
        // Load callers/callees for this symbol
        let callers = load_callers(conn, &symbol.name, &symbol.file).await;
        let callees = load_callees(conn, &symbol.name, &symbol.file).await;
        let co_changes = co_change_map.get(&symbol.file).cloned().unwrap_or_default();

        // Load doc comment if present (already clean text — markers stripped at index time
        // by Language::extract_docstring; stored as `doc:<text>` in symbol_attributes)
        let doc = load_doc_comment(conn, &symbol.name, &symbol.file).await;

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

    // Flush remainder
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

    // Reclaim space after a full rebuild.
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
            // Wrap the entire batch in a single transaction to avoid per-row
            // transaction overhead (massive speedup for 9000+ symbols).
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
    let sql = if changed_paths.is_some() {
        // We'll filter after loading to avoid complex dynamic SQL
        "SELECT rowid, file, name, kind, start_line, end_line, parent FROM symbols".to_string()
    } else {
        "SELECT rowid, file, name, kind, start_line, end_line, parent FROM symbols".to_string()
    };

    let mut rows = conn.query(&sql, ()).await?;
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

/// Load top callers for a symbol (returns caller symbol names, up to 10).
async fn load_callers(conn: &Connection, symbol_name: &str, _file: &str) -> Vec<String> {
    let Ok(mut rows) = conn
        .query(
            "SELECT caller_symbol, COUNT(*) as cnt FROM calls WHERE callee_name = ?1 GROUP BY caller_symbol ORDER BY cnt DESC LIMIT 10",
            libsql::params![symbol_name],
        )
        .await
    else {
        return Vec::new();
    };

    let mut callers = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        if let Ok(name) = row.get::<String>(0) {
            callers.push(name);
        }
    }
    callers
}

/// Load callees for a symbol (returns callee names, up to 10).
async fn load_callees(conn: &Connection, symbol_name: &str, file: &str) -> Vec<String> {
    let Ok(mut rows) = conn
        .query(
            "SELECT callee_name FROM calls WHERE caller_symbol = ?1 AND caller_file = ?2 LIMIT 10",
            libsql::params![symbol_name, file],
        )
        .await
    else {
        return Vec::new();
    };

    let mut callees = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        if let Ok(name) = row.get::<String>(0) {
            callees.push(name);
        }
    }
    callees
}

/// Load co-change neighbors for all files as a map: file → [neighbor_files].
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
            // co_change_edges table may not exist yet — return empty map
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

/// Attempt to load a doc comment for a symbol from the `symbol_attributes` table.
/// Returns `None` if no doc comment is stored.
async fn load_doc_comment(conn: &Connection, symbol_name: &str, file: &str) -> Option<String> {
    let mut rows = conn
        .query(
            "SELECT attribute FROM symbol_attributes WHERE name = ?1 AND file = ?2 AND attribute LIKE 'doc:%'",
            libsql::params![symbol_name, file],
        )
        .await
        .ok()?;

    let mut parts = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        if let Ok(attr) = row.get::<String>(0)
            && let Some(doc) = attr.strip_prefix("doc:")
        {
            parts.push(doc.to_string());
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}
