//! Report types, CLI service, and search logic for the `normalize search` verb.
//!
//! `SemanticCliService` provides the top-level `search` verb (mounted by the
//! main crate under the `cli` feature). It delegates to `run_search`, which
//! embeds the query and ranks stored chunk embeddings. `run_context_search`
//! backs `normalize context --semantic`.

use crate::config::EmbeddingsConfig;
use crate::search::SearchHit;
use crate::store;
use crate::vec_ext::VecConnection;
use normalize_output::OutputFormatter;
use serde::{Deserialize, Serialize};
use server_less::cli;
use std::path::PathBuf;

#[cfg(feature = "cli")]
use schemars::JsonSchema;

/// One result entry returned by `normalize search`.
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

/// Report returned by `normalize search`.
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

/// Core search logic, called from `SemanticCliService::search` (the `search` verb).
///
/// `root` is the project root directory; `query` is the natural-language query.
/// `top_k` is the max number of results to return.
///
/// Returns a human-readable error string (suitable for `Err(...)` in a service method).
#[cfg(feature = "embeddings")]
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

/// One result entry returned by `normalize context --semantic`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct ContextSearchEntry {
    /// Relative path to the context block file.
    pub path: String,
    /// The full text content of this context block section.
    pub content: String,
    /// Cosine similarity to the query (before staleness penalty).
    pub similarity: f32,
    /// Final re-ranked score.
    pub score: f32,
}

impl From<SearchHit> for ContextSearchEntry {
    fn from(h: SearchHit) -> Self {
        Self {
            path: h.source_path,
            content: h.chunk_text,
            similarity: h.similarity,
            score: h.score,
        }
    }
}

/// Report returned by `normalize context --semantic`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct ContextSearchReport {
    /// The query string as submitted.
    pub query: String,
    /// Embedding model used.
    pub model: String,
    /// Ranked context block results, best first.
    pub results: Vec<ContextSearchEntry>,
    /// Total context embeddings scanned.
    pub total_scanned: usize,
}

impl OutputFormatter for ContextSearchReport {
    fn format_text(&self) -> String {
        if self.results.is_empty() {
            return format!(
                "No context blocks found for query: {}\n(Hint: run `normalize structure rebuild` to populate embeddings for context blocks)",
                self.query
            );
        }

        let mut out = String::new();
        for r in &self.results {
            out.push_str(&r.content);
            if !r.content.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        // Remove trailing blank line
        while out.ends_with("\n\n") {
            out.pop();
        }
        out
    }
}

/// Core search logic for `normalize context --semantic`.
///
/// Searches the embeddings index restricted to `source_type = "context"` blocks.
/// Returns the top-k most relevant context blocks by cosine similarity.
#[cfg(feature = "embeddings")]
pub async fn run_context_search(
    root: &std::path::Path,
    query: String,
    top_k: usize,
) -> Result<ContextSearchReport, String> {
    let config = load_embeddings_config(root);

    if !config.enabled {
        return Err(
            "Semantic search not enabled. Add [embeddings] enabled = true to .normalize/config.toml, then run `normalize structure rebuild`.".to_string(),
        );
    }

    let idx = crate::open_index(root)
        .await
        .map_err(|e| format!("Failed to open index: {e}"))?;

    let conn = idx.connection();

    let db_path = root.join(".normalize").join("index.sqlite");
    let vec_conn: Option<VecConnection> = VecConnection::open(&db_path);

    store::ensure_schema(conn)
        .await
        .map_err(|e| format!("Schema error: {e}"))?;

    let dims = crate::embedder::dims_for_model(&config.model).unwrap_or(768);
    store::ensure_vec_schema(conn, dims, vec_conn.as_ref()).await;

    let mut embedder = crate::embedder::Embedder::load(&config.model, None)
        .map_err(|e| format!("Failed to load embedding model: {e}"))?;

    let query_vec = embedder
        .embed_one(&query)
        .map_err(|e| format!("Embedding failed: {e}"))?;

    // For context blocks, use the brute-force path scoped to source_type='context'.
    // ANN search returns all source types; post-filtering would waste candidates.
    // Since context blocks are few, brute-force is fast enough.
    let candidates = store::load_embeddings_for_type(conn, &config.model, "context")
        .await
        .map_err(|e| format!("Failed to load context embeddings: {e}"))?;

    if candidates.is_empty() {
        return Ok(ContextSearchReport {
            query,
            model: config.model,
            results: Vec::new(),
            total_scanned: 0,
        });
    }

    let total_scanned = candidates.len();
    let hits = crate::search::rerank(&query_vec, candidates, top_k);

    Ok(ContextSearchReport {
        query,
        model: config.model,
        results: hits.into_iter().map(ContextSearchEntry::from).collect(),
        total_scanned,
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

// ---------------------------------------------------------------------------
// CLI service — `normalize search`
// ---------------------------------------------------------------------------

/// CLI service implementing the top-level `normalize search` verb (semantic /
/// vector search over the structural index).
pub struct SemanticCliService;

impl SemanticCliService {
    /// Create a new semantic search service.
    pub fn new() -> Self {
        Self
    }

    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

impl Default for SemanticCliService {
    fn default() -> Self {
        Self::new()
    }
}

#[cli(
    name = "search",
    version = "0.3.2",
    description = "Semantic (vector) search over the code index. Ranks symbols, docs, and commits by meaning rather than by name. Requires embeddings — enable `[embeddings]` in .normalize/config.toml and run `structure rebuild`."
)]
impl SemanticCliService {
    /// Search the semantic index for chunks matching a natural-language query.
    ///
    /// Embeds the query and ranks stored chunk embeddings by cosine similarity,
    /// re-ranked by git staleness. Uses the ANN index when available, falling
    /// back to a brute-force scan otherwise.
    ///
    /// Requires the embeddings index: set `[embeddings] enabled = true` in
    /// `.normalize/config.toml`, then run `normalize structure rebuild`.
    ///
    /// Examples:
    ///   normalize search "retry logic with backoff"
    ///   normalize search "parse config file" --limit 5
    #[cli(default, display_with = "display_output")]
    pub async fn search(
        &self,
        #[param(positional, help = "Natural-language query")] query: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(
            short = 'n',
            name = "limit",
            help = "Maximum number of results to return (default 10)"
        )]
        limit: Option<usize>,
    ) -> Result<SearchReport, String> {
        let root = match root {
            Some(r) => PathBuf::from(r),
            None => std::env::current_dir().map_err(|e| e.to_string())?,
        };
        let top_k = limit.unwrap_or(10);
        run_search(&root, query, top_k).await
    }
}
