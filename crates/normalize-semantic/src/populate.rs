//! Embedding population: walk symbols from the structural index and embed them.
//!
//! Called after `structure rebuild` when `embeddings.enabled = true`.
//! For incremental rebuilds, only symbols in changed files are re-embedded.

use crate::chunks::{SymbolRow, build_symbol_chunk};
use crate::config::EmbeddingsConfig;
use crate::embedder::{Embedder, encode_vector};
use crate::store;
use libsql::Connection;
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
pub async fn populate_embeddings(
    conn: &Connection,
    config: &EmbeddingsConfig,
    changed_paths: Option<&[String]>,
    head_commit: Option<&str>,
) -> anyhow::Result<PopulateStats> {
    store::ensure_schema(conn).await?;

    let mut embedder = Embedder::load(&config.model, None)?;
    info!(model = %config.model, dims = embedder.dimensions, "Embedding model loaded");

    let mut stats = PopulateStats::default();

    // Load all symbols
    let symbols = load_symbols(conn, changed_paths).await?;

    if symbols.is_empty() {
        return Ok(stats);
    }

    // If incremental, delete old embeddings for changed paths first
    if let Some(paths) = changed_paths {
        for path in paths {
            store::delete_embeddings_for_path(conn, path).await?;
        }
    }

    // Build context for co-change lookups (file → co-change neighbors)
    let co_change_map = load_co_change_map(conn).await?;

    // Process in batches
    let mut batch_symbols: Vec<SymbolRow> = Vec::new();
    let mut batch_texts: Vec<String> = Vec::new();

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

        batch_symbols.push(symbol);
        batch_texts.push(chunk_text);

        if batch_texts.len() >= EMBED_BATCH_SIZE {
            flush_batch(
                conn,
                &mut embedder,
                &batch_symbols,
                &batch_texts,
                head_commit,
                &config.model,
                &mut stats,
            )
            .await;
            batch_symbols.clear();
            batch_texts.clear();
        }
    }

    // Flush remainder
    if !batch_texts.is_empty() {
        flush_batch(
            conn,
            &mut embedder,
            &batch_symbols,
            &batch_texts,
            head_commit,
            &config.model,
            &mut stats,
        )
        .await;
    }

    info!(
        embedded = stats.symbols_embedded,
        errors = stats.errors,
        "Embedding population complete"
    );

    Ok(stats)
}

/// Flush one batch of symbols through the embedder and store.
async fn flush_batch(
    conn: &Connection,
    embedder: &mut Embedder,
    symbols: &[SymbolRow],
    texts: &[String],
    head_commit: Option<&str>,
    model_name: &str,
    stats: &mut PopulateStats,
) {
    let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    match embedder.embed_batch(&text_refs) {
        Ok(vectors) => {
            for (sym, (text, vec)) in symbols.iter().zip(texts.iter().zip(vectors.iter())) {
                let blob = encode_vector(vec);
                match store::upsert_embedding(
                    conn,
                    "symbol",
                    &sym.file,
                    Some(sym.rowid),
                    model_name,
                    head_commit,
                    0.0, // staleness computed separately in future enhancement
                    text,
                    &blob,
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
