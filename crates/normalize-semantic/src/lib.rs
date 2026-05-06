//! Semantic retrieval layer for normalize.
//!
//! This crate provides vector embeddings over structurally-derived chunks
//! (symbols + doc comments + caller/callee context + co-change neighbors),
//! stored in SQLite alongside the structural index, queryable by meaning
//! rather than by name.
//!
//! ## Architecture
//!
//! - **[`config`]** -- `EmbeddingsConfig` (`[embeddings]` section of config.toml)
//! - **[`chunks`]** -- context window construction from index rows
//! - **[`embedder`]** -- fastembed wrapper (ONNX, no server required)
//! - **[`schema`]** -- SQLite DDL for the `embeddings` table
//! - **[`store`]** -- read/write embeddings to/from SQLite
//! - **[`search`]** -- ANN search + staleness re-ranking
//! - **[`populate`]** -- walk the structural index and embed symbols, docs, and commits
//! - **[`service`]** -- CLI service (`normalize structure search`) -- `cli` feature
//!
//! ## Usage
//!
//! After `structure rebuild`, call [`populate::populate_embeddings`] with the
//! active `FileIndex` connection to generate and store embeddings.
//!
//! For markdown and commit embeddings, call [`populate::populate_markdown_docs`]
//! and [`populate::populate_commit_messages`] respectively.
//! For `.normalize/context/` block embeddings, call [`populate::populate_context_blocks`].
//!
//! To search, call [`service::run_search`] (all source types) or
//! [`service::run_context_search`] (context blocks only), or use
//! [`store::load_all_embeddings`] + [`search::rerank`] directly.

pub mod chunks;
pub mod config;
pub mod embedder;
pub mod git_staleness;
pub mod populate;
pub mod schema;
pub mod search;
pub mod store;
pub mod vec_ext;

#[cfg(feature = "cli")]
pub mod service;

// Re-export the key public types for convenience.
pub use config::EmbeddingsConfig;
pub use populate::{
    DEFAULT_MAX_COMMITS, PopulateStats, populate_commit_messages, populate_context_blocks,
    populate_embeddings, populate_incremental_for_paths, populate_markdown_docs,
};
pub use search::SearchHit;

use libsql::Connection;
use normalize_facts::FileIndex;

/// Open the index and return a reference to its SQLite connection.
/// Convenience helper used by populate and service modules.
pub async fn open_index(root: &std::path::Path) -> Result<FileIndex, libsql::Error> {
    let normalize_dir = root.join(".normalize");
    let db_path = normalize_dir.join("index.sqlite");
    FileIndex::open(&db_path, root).await
}

/// Ensure the embeddings schema exists in the given connection.
/// Safe to call multiple times (all DDL uses `IF NOT EXISTS`).
pub async fn ensure_schema(conn: &Connection) -> Result<(), libsql::Error> {
    store::ensure_schema(conn).await
}
