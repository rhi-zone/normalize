//! SQLite schema for the embeddings table.
//!
//! Vectors are stored in the same SQLite database as the structural index
//! (`.normalize/index.sqlite`). The `embeddings` table holds one row per
//! embedded chunk; the raw f32 vector is stored as a BLOB alongside staleness
//! metadata used for query-time re-ranking.

/// DDL for the embeddings table.
///
/// The table is created lazily when embedding is first enabled so it does not
/// affect repos that never turn on `embeddings.enabled`.
pub const CREATE_EMBEDDINGS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS embeddings (
    id          INTEGER PRIMARY KEY,
    source_type TEXT NOT NULL,   -- 'symbol' | 'doc' | 'commit' | 'cluster'
    source_path TEXT NOT NULL,   -- relative file path
    source_id   INTEGER,         -- FK into symbols table where applicable
    model       TEXT NOT NULL,   -- embedding model name (triggers invalidation on change)
    last_commit TEXT,            -- git HEAD SHA when last embedded
    staleness   REAL NOT NULL DEFAULT 0.0,
    chunk_text  TEXT NOT NULL,   -- the text that was embedded (for debugging / re-use)
    embedding   BLOB NOT NULL    -- packed f32 array, length = model dimensions
)";

pub const CREATE_EMBEDDINGS_IDX_SOURCE: &str = "
CREATE INDEX IF NOT EXISTS idx_embeddings_source
    ON embeddings(source_type, source_path)";

pub const CREATE_EMBEDDINGS_IDX_MODEL: &str = "
CREATE INDEX IF NOT EXISTS idx_embeddings_model
    ON embeddings(model)";
