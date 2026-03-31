//! SQLite schema for the embeddings table and sqlite-vec ANN index.
//!
//! Vectors are stored in the same SQLite database as the structural index
//! (`.normalize/index.sqlite`). The `embeddings` table holds one row per
//! embedded chunk; the raw f32 vector is stored as a BLOB alongside staleness
//! metadata used for query-time re-ranking.
//!
//! When the sqlite-vec extension is loaded, a companion `vec_embeddings`
//! virtual table (backed by `vec0`) mirrors the vector data and enables
//! approximate nearest-neighbor search.  The `rowid` of `vec_embeddings` is
//! kept in sync with `embeddings.id` so they can be JOIN-ed freely.

/// DDL for the embeddings table.
///
/// The table is created lazily when embedding is first enabled so it does not
/// affect repos that never turn on `embeddings.enabled`.
///
/// The UNIQUE constraint on `(source_type, source_path, source_id)` allows
/// `INSERT OR REPLACE` for incremental updates without a delete-then-insert
/// round-trip.
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
    embedding   BLOB NOT NULL,   -- packed f32 array, length = model dimensions
    UNIQUE(source_type, source_path, source_id)
)";

/// DDL statements to drop embedding tables for a full rebuild.
pub const DROP_EMBEDDINGS_TABLE: &str = "DROP TABLE IF EXISTS embeddings";
pub const DROP_VEC_EMBEDDINGS_TABLE: &str = "DROP TABLE IF EXISTS vec_embeddings";
pub const DROP_EMBEDDINGS_IDX_SOURCE: &str = "DROP INDEX IF EXISTS idx_embeddings_source";
pub const DROP_EMBEDDINGS_IDX_MODEL: &str = "DROP INDEX IF EXISTS idx_embeddings_model";

pub const CREATE_EMBEDDINGS_IDX_SOURCE: &str = "
CREATE INDEX IF NOT EXISTS idx_embeddings_source
    ON embeddings(source_type, source_path)";

pub const CREATE_EMBEDDINGS_IDX_MODEL: &str = "
CREATE INDEX IF NOT EXISTS idx_embeddings_model
    ON embeddings(model)";

/// DDL for the sqlite-vec ANN virtual table.
///
/// This is created only when the sqlite-vec extension is available (i.e. after
/// [`crate::vec_ext::register_vec_extension`] has been called and a connection
/// opened).  The dimension count is passed at table-creation time; the default
/// of 768 matches `nomic-embed-text-v1.5`.
///
/// `vec0` tables store only `(rowid, vector)`.  Metadata is fetched by
/// JOIN-ing back to the `embeddings` table using the `rowid`.
pub fn create_vec_embeddings_ddl(dims: usize) -> String {
    format!("CREATE VIRTUAL TABLE IF NOT EXISTS vec_embeddings USING vec0(embedding float[{dims}])")
}
