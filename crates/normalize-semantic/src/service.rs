//! Report types and search logic for `normalize structure search`.
//!
//! The CLI method itself lives in `crates/normalize/src/service/facts.rs` so it
//! can be added to `FactsService` under the `structure` subcommand group.
//! This module exports the report struct and the `run_search` function that the
//! method delegates to.

use crate::config::EmbeddingsConfig;
use crate::search::SearchHit;
use crate::store;
use crate::vec_ext::VecConnection;
use normalize_output::OutputFormatter;
use serde::{Deserialize, Serialize};

#[cfg(feature = "cli")]
use schemars::JsonSchema;

/// One result entry returned by `normalize structure search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct SearchResultEntry {
    /// Relative file path containing the source.
    pub path: String,
    /// Source type: "symbol", "doc", "commit", or "cluster".
    pub source_type: String,
    /// The chunk text that was embedded.
    pub chunk_text: String,
    /// Cosine similarity to the query (before staleness penalty).
    pub similarity: f32,
    /// Staleness score in [0, 1].
    pub staleness: f32,
    /// Final re-ranked score.
    pub score: f32,
    /// Git commit hash when this chunk was last embedded.
    pub last_commit: Option<String>,
}

impl From<SearchHit> for SearchResultEntry {
    fn from(h: SearchHit) -> Self {
        Self {
            path: h.source_path,
            source_type: h.source_type,
            chunk_text: h.chunk_text,
            similarity: h.similarity,
            staleness: h.staleness,
            score: h.score,
            last_commit: h.last_commit,
        }
    }
}

/// Report returned by `normalize structure search`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct SearchReport {
    /// The query string as submitted.
    pub query: String,
    /// Embedding model used.
    pub model: String,
    /// Ranked results, best first.
    pub results: Vec<SearchResultEntry>,
    /// Total embeddings scanned (ANN candidate count or full index size).
    pub total_scanned: usize,
    /// `true` when the ANN index (`vec_embeddings`) was used; `false` when
    /// the brute-force in-memory path was used instead.
    pub ann_used: bool,
}

impl OutputFormatter for SearchReport {
    fn format_text(&self) -> String {
        if self.results.is_empty() {
            return format!(
                "No results for query: {}\n(Hint: run `normalize structure rebuild` to populate embeddings)",
                self.query
            );
        }

        let search_mode = if self.ann_used { "ANN" } else { "brute-force" };
        let mut out = format!(
            "Semantic search results for: \"{}\"\nModel: {} — scanned {} embeddings ({})\n\n",
            self.query, self.model, self.total_scanned, search_mode
        );

        for (i, r) in self.results.iter().enumerate() {
            out.push_str(&format!(
                "{}. [score={:.3}] {} ({})\n",
                i + 1,
                r.score,
                r.path,
                r.source_type,
            ));
            // Show first line of chunk_text as a snippet
            let snippet = r
                .chunk_text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect::<String>();
            out.push_str(&format!("   {}\n", snippet));
        }

        out
    }
}

/// Core search logic, called from `FactsService::search` in the main crate.
///
/// `root` is the project root directory; `query` is the natural-language query.
/// `top_k` is the max number of results to return.
///
/// Returns a human-readable error string (suitable for `Err(...)` in a service method).
pub async fn run_search(
    root: &std::path::Path,
    query: String,
    top_k: usize,
) -> Result<SearchReport, String> {
    let config = load_embeddings_config(root);

    if !config.enabled {
        let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
        if is_tty {
            eprintln!(
                "Semantic search is not enabled. Add to .normalize/config.toml:\n\n  [embeddings]\n  enabled = true\n"
            );
        } else {
            eprintln!(
                "error: semantic search not enabled (embeddings.enabled = false in config.toml)"
            );
        }
        return Err("Semantic search not enabled.".to_string());
    }

    let idx = crate::open_index(root)
        .await
        .map_err(|e| format!("Failed to open index: {e}"))?;

    let conn = idx.connection();

    // Open a parallel raw connection with sqlite-vec registered for ANN operations.
    let db_path = root.join(".normalize").join("index.sqlite");
    let vec_conn: Option<VecConnection> = VecConnection::open(&db_path);

    store::ensure_schema(conn)
        .await
        .map_err(|e| format!("Schema error: {e}"))?;

    // Try to create the ANN virtual table.  The dimension count comes from the
    // embedder config; default 768 matches nomic-embed-text-v1.5.
    let dims = crate::embedder::dims_for_model(&config.model).unwrap_or(768);
    store::ensure_vec_schema(conn, dims, vec_conn.as_ref()).await;

    let total = store::count_embeddings(conn, &config.model)
        .await
        .map_err(|e| format!("DB error: {e}"))?;

    if total == 0 {
        let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
        if is_tty {
            eprintln!(
                "No embeddings found. Run `normalize structure rebuild` to populate the semantic index."
            );
        } else {
            eprintln!(
                "error: no embeddings for model '{}'. Run `normalize structure rebuild` first.",
                config.model
            );
        }
        return Ok(SearchReport {
            query,
            model: config.model,
            results: Vec::new(),
            total_scanned: 0,
            ann_used: false,
        });
    }

    let mut embedder = crate::embedder::Embedder::load(&config.model, None)
        .map_err(|e| format!("Failed to load embedding model: {e}"))?;

    let query_vec = embedder
        .embed_one(&query)
        .map_err(|e| format!("Embedding failed: {e}"))?;

    // Try ANN path first (sqlite-vec `vec_embeddings` virtual table).
    // Fall back to brute-force if the extension isn't loaded or the table
    // doesn't exist yet.
    let query_bytes = crate::embedder::encode_vector(&query_vec);
    let ann_candidate_count = std::cmp::max(store::ANN_CANDIDATE_COUNT, top_k);

    let (candidates, ann_used) = if let Some(ann_results) = store::ann_search(
        conn,
        &config.model,
        &query_bytes,
        ann_candidate_count,
        vec_conn.as_ref(),
    )
    .await
    .filter(|r| !r.is_empty())
    {
        (ann_results, true)
    } else {
        let all = store::load_all_embeddings(conn, &config.model)
            .await
            .map_err(|e| format!("Failed to load embeddings: {e}"))?;
        (all, false)
    };

    let total_scanned = candidates.len();

    let hits = crate::search::rerank(&query_vec, candidates, top_k);

    Ok(SearchReport {
        query,
        model: config.model,
        results: hits.into_iter().map(SearchResultEntry::from).collect(),
        total_scanned,
        ann_used,
    })
}

/// Load embeddings config from the project's config.toml.
/// Falls back to default (disabled) if config is missing or malformed.
pub fn load_embeddings_config(root: &std::path::Path) -> EmbeddingsConfig {
    let config_path = root.join(".normalize").join("config.toml");
    let Ok(contents) = std::fs::read_to_string(&config_path) else {
        return EmbeddingsConfig::default();
    };
    #[derive(serde::Deserialize, Default)]
    struct PartialConfig {
        #[serde(default)]
        embeddings: EmbeddingsConfig,
    }
    toml::from_str::<PartialConfig>(&contents)
        .map(|c| c.embeddings)
        .unwrap_or_default()
}
