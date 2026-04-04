//! SQLite-backed per-file findings cache.
//!
//! Stored at `<project_root>/.normalize/findings-cache.sqlite`.
//! Keyed by `(path, engine)`: each engine stores its own findings per file.
//! A `config_hash` column invalidates the entry when rule config changes.

use rusqlite::{Connection, params};
use std::path::Path;

/// SQLite-backed per-file findings cache.
///
/// Stored at `<project_root>/.normalize/findings-cache.sqlite`.
/// Keyed by `(path, engine)`: each engine stores its own findings per file.
/// A `config_hash` column invalidates the entry when rule config changes.
pub struct FindingsCache {
    conn: Connection,
}

impl FindingsCache {
    /// Open (or create) the cache database at `<project_root>/.normalize/findings-cache.sqlite`.
    ///
    /// Returns an in-memory fallback if the database cannot be opened (e.g. permission error),
    /// so callers never need to handle failure — the cost is just a cold run.
    pub fn open(project_root: &Path) -> Self {
        let dir = project_root.join(".normalize");
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("findings-cache.sqlite");

        let conn = Connection::open(&db_path)
            .or_else(|_| Connection::open_in_memory())
            .expect("failed to open in-memory SQLite connection");

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS findings_cache (
                path TEXT NOT NULL,
                engine TEXT NOT NULL,
                mtime_nanos INTEGER NOT NULL,
                config_hash TEXT NOT NULL,
                findings_json TEXT NOT NULL,
                PRIMARY KEY (path, engine)
            );",
        )
        .ok();

        Self { conn }
    }

    /// Return cached findings JSON blob if `(path, mtime_nanos, config_hash, engine)` all match.
    pub fn get(
        &self,
        path: &str,
        mtime_nanos: u64,
        config_hash: &str,
        engine: &str,
    ) -> Option<String> {
        self.conn
            .query_row(
                "SELECT findings_json FROM findings_cache
                 WHERE path = ?1 AND engine = ?2 AND mtime_nanos = ?3 AND config_hash = ?4",
                params![path, engine, mtime_nanos as i64, config_hash],
                |row| row.get::<_, String>(0),
            )
            .ok()
    }

    /// Store findings for a file. Called after a fresh analysis.
    pub fn put(
        &self,
        path: &str,
        mtime_nanos: u64,
        config_hash: &str,
        engine: &str,
        findings_json: &str,
    ) {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO findings_cache (path, engine, mtime_nanos, config_hash, findings_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![path, engine, mtime_nanos as i64, config_hash, findings_json],
            )
            .ok();
    }

    /// No-op — SQLite writes are immediate. Exists for API symmetry.
    pub fn flush(&self) {}
}

/// Get the mtime of a file in nanoseconds since UNIX epoch cast to `u64`, or 0 on failure.
///
/// `u64` is used rather than `u128` so the value fits in SQLite's `INTEGER` (64-bit signed).
/// Nanosecond precision avoids false cache hits when a file is modified within the same second.
pub fn file_mtime_nanos(path: &Path) -> u64 {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        })
        .unwrap_or(0)
}
