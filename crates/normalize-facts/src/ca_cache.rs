use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(crate) struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<bincode::Error> for Error {
    fn from(e: bincode::Error) -> Self {
        Error(e.to_string())
    }
}

/// Content-addressed extraction cache. Keyed by `(blake3_hash, extractor_version, grammar)`.
/// One shared DB across the whole daemon process; safe to clone and share across threads.
#[derive(Clone)]
pub(crate) struct CaCache {
    conn: Arc<Mutex<Connection>>,
    max_size_bytes: u64,
}

impl CaCache {
    /// Open (or create) the CA cache at the given path. Creates parent directories.
    pub(crate) fn open(path: &Path, max_size_bytes: u64) -> Result<Self, Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error(format!("create_dir_all: {e}")))?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS ca_entries (
               hash      BLOB    NOT NULL,
               extr_ver  TEXT    NOT NULL,
               grammar   TEXT    NOT NULL,
               payload   BLOB    NOT NULL,
               last_used INTEGER NOT NULL DEFAULT (strftime('%s','now')),
               PRIMARY KEY (hash, extr_ver, grammar)
             ) WITHOUT ROWID;",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            max_size_bytes,
        })
    }

    /// Default path: `~/.config/normalize/ca-cache.sqlite`.
    pub(crate) fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("normalize")
            .join("ca-cache.sqlite")
    }

    /// Look up a cached payload. Returns `None` on miss or version mismatch.
    pub(crate) fn get<T: DeserializeOwned>(
        &self,
        hash: &[u8],
        extr_ver: &str,
        grammar: &str,
    ) -> Result<Option<T>, Error> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = unix_now();
        let result: Option<Vec<u8>> = conn
            .query_row(
                "SELECT payload FROM ca_entries WHERE hash = ?1 AND extr_ver = ?2 AND grammar = ?3",
                params![hash, extr_ver, grammar],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(bytes) = &result {
            // Touch last_used (best-effort — ignore errors)
            let _ = conn.execute(
                "UPDATE ca_entries SET last_used = ?1 WHERE hash = ?2 AND extr_ver = ?3 AND grammar = ?4",
                params![now, hash, extr_ver, grammar],
            );
            let value: T = bincode::deserialize(bytes)?;
            return Ok(Some(value));
        }
        Ok(None)
    }

    /// Store a payload. Silently overwrites existing entries with the same key.
    pub(crate) fn put<T: Serialize>(
        &self,
        hash: &[u8],
        extr_ver: &str,
        grammar: &str,
        value: &T,
    ) -> Result<(), Error> {
        let bytes = bincode::serialize(value)?;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let now = unix_now();
        conn.execute(
            "INSERT OR REPLACE INTO ca_entries (hash, extr_ver, grammar, payload, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![hash, extr_ver, grammar, bytes, now],
        )?;
        Ok(())
    }

    /// Remove entries for outdated extractor versions. Call once at startup.
    pub(crate) fn gc_stale_versions(&self, current_extr_ver: &str) -> Result<usize, Error> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let deleted = conn.execute(
            "DELETE FROM ca_entries WHERE extr_ver != ?1",
            params![current_extr_ver],
        )?;
        Ok(deleted)
    }

    /// Evict oldest-accessed entries until DB file size is under `max_size_bytes`.
    /// Uses page_count * page_size as the size estimate; runs VACUUM after deletion.
    #[allow(dead_code)] // not yet wired into the refresh path; retained for future use
    pub(crate) fn evict_if_over_limit(&self) -> Result<(), Error> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let size: u64 = conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n as u64)
            .unwrap_or(0);
        if size <= self.max_size_bytes {
            return Ok(());
        }
        // Evict ~10% at a time to avoid over-deleting
        let target = self.max_size_bytes * 9 / 10;
        loop {
            let current: u64 = conn
                .query_row(
                    "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|n| n as u64)
                .unwrap_or(0);
            if current <= target {
                break;
            }
            let deleted = conn.execute(
                "DELETE FROM ca_entries WHERE (hash, extr_ver, grammar) IN (
                   SELECT hash, extr_ver, grammar FROM ca_entries ORDER BY last_used ASC LIMIT 100
                 )",
                [],
            )?;
            if deleted == 0 {
                break;
            }
        }
        conn.execute_batch("VACUUM;")?;
        Ok(())
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::NamedTempFile;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Payload {
        symbols: Vec<String>,
        count: u32,
    }

    fn temp_cache() -> CaCache {
        let f = NamedTempFile::new().unwrap();
        let path = f.path().to_path_buf();
        std::mem::forget(f); // keep file alive
        CaCache::open(&path, 1024 * 1024 * 1024).unwrap()
    }

    #[test]
    fn round_trip() {
        let cache = temp_cache();
        let hash = blake3::hash(b"hello world");
        let payload = Payload {
            symbols: vec!["foo".into(), "bar".into()],
            count: 42,
        };
        cache.put(hash.as_bytes(), "v1", "rust", &payload).unwrap();
        let got: Option<Payload> = cache.get(hash.as_bytes(), "v1", "rust").unwrap();
        assert_eq!(got, Some(payload));
    }

    #[test]
    fn version_mismatch_returns_none() {
        let cache = temp_cache();
        let hash = blake3::hash(b"hello world");
        let payload = Payload {
            symbols: vec![],
            count: 1,
        };
        cache.put(hash.as_bytes(), "v1", "rust", &payload).unwrap();
        let got: Option<Payload> = cache.get(hash.as_bytes(), "v2", "rust").unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn gc_stale_versions() {
        let cache = temp_cache();
        let hash = blake3::hash(b"test");
        let payload = Payload {
            symbols: vec![],
            count: 0,
        };
        cache.put(hash.as_bytes(), "old", "rust", &payload).unwrap();
        cache
            .put(hash.as_bytes(), "current", "rust", &payload)
            .unwrap();
        let deleted = cache.gc_stale_versions("current").unwrap();
        assert_eq!(deleted, 1);
        let got: Option<Payload> = cache.get(hash.as_bytes(), "old", "rust").unwrap();
        assert_eq!(got, None);
        let got: Option<Payload> = cache.get(hash.as_bytes(), "current", "rust").unwrap();
        assert!(got.is_some());
    }

    #[test]
    fn eviction_under_limit() {
        let cache = temp_cache();
        // Put some entries
        for i in 0u32..10 {
            let hash = blake3::hash(i.to_le_bytes().as_slice());
            let payload = Payload {
                symbols: vec!["x".repeat(1000)],
                count: i,
            };
            cache.put(hash.as_bytes(), "v1", "rust", &payload).unwrap();
        }
        // evict_if_over_limit with generous limit should do nothing
        cache.evict_if_over_limit().unwrap();
    }
}
