use libsql::{Builder, Connection, Database, params};
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::runtime::{Handle, Runtime};

#[derive(Debug)]
pub(crate) struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<libsql::Error> for Error {
    fn from(e: libsql::Error) -> Self {
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
///
/// Backed by libsql. The cache owns a dedicated current-thread tokio runtime so its
/// public API stays synchronous (callers can't reasonably switch to async without
/// cascading through the entire native-rules surface).
#[derive(Clone)]
pub(crate) struct CaCache {
    inner: Arc<Inner>,
    max_size_bytes: u64,
}

struct Inner {
    conn: Connection,
    /// Keep the Database alive for the lifetime of the connection.
    #[allow(dead_code)]
    db: Database,
    /// Owned runtime — only present when we are not running inside an existing
    /// tokio runtime. If `None`, calls use `Handle::current()` + `block_in_place`.
    runtime: Option<Runtime>,
}

impl Inner {
    fn block_on<F: Future + Send>(&self, fut: F) -> F::Output
    where
        F::Output: Send,
    {
        block_on_helper(&self.runtime, fut)
    }
}

/// Drive `fut` to completion, choosing a strategy based on the *current* thread's
/// tokio context — not the context at cache-construction time.
///
/// Why we ignore the cached `runtime` when we're already inside a tokio runtime:
/// the cache may have been opened from a sync context (so it owns a current-thread
/// runtime), then later called from a `#[tokio::test]` or any other tokio task on
/// a different thread. Calling `cached_rt.block_on(...)` from inside another runtime
/// panics with "Cannot start a runtime from within a runtime". So the call-site
/// context always wins:
///
/// - Inside a multi-threaded runtime: `block_in_place` + `Handle::current().block_on`.
/// - Inside a current-thread runtime: spawn a scoped OS thread with its own runtime
///   (block_in_place would panic on a current-thread runtime).
/// - Not inside any runtime: use the cached owned runtime if we have one; otherwise
///   spawn a scoped OS thread.
fn block_on_helper<F: Future + Send>(runtime: &Option<Runtime>, fut: F) -> F::Output
where
    F::Output: Send,
{
    if let Ok(handle) = Handle::try_current() {
        return match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(fut))
            }
            _ => spawn_scoped_runtime(fut),
        };
    }
    if let Some(rt) = runtime {
        return rt.block_on(fut);
    }
    spawn_scoped_runtime(fut)
}

/// Drive `fut` on a freshly-built current-thread runtime hosted on a scoped OS thread.
/// Used when the calling thread is unsuitable (already inside a current-thread runtime,
/// or we have no cached runtime to fall back on).
fn spawn_scoped_runtime<F: Future + Send>(fut: F) -> F::Output
where
    F::Output: Send,
{
    std::thread::scope(|s| {
        s.spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build tokio runtime worker thread");
            rt.block_on(fut)
        })
        .join()
        .expect("libsql worker thread panicked")
    })
}

/// Build a current-thread tokio runtime if we are not already inside one.
fn maybe_build_runtime() -> Result<Option<Runtime>, Error> {
    if Handle::try_current().is_ok() {
        return Ok(None);
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map(Some)
        .map_err(|e| Error(format!("tokio runtime: {e}")))
}

impl CaCache {
    /// Open (or create) the CA cache at the given path. Creates parent directories.
    pub(crate) fn open(path: &Path, max_size_bytes: u64) -> Result<Self, Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error(format!("create_dir_all: {e}")))?;
        }
        let runtime = maybe_build_runtime()?;
        let init = async {
            let db = Builder::new_local(path).build().await?;
            let conn = db.connect()?;
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=NORMAL;
                 PRAGMA busy_timeout=5000;
                 CREATE TABLE IF NOT EXISTS ca_entries (
                   hash      BLOB    NOT NULL,
                   extr_ver  TEXT    NOT NULL,
                   grammar   TEXT    NOT NULL,
                   payload   BLOB    NOT NULL,
                   last_used INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                   PRIMARY KEY (hash, extr_ver, grammar)
                 ) WITHOUT ROWID;",
            )
            .await?;
            Ok::<_, libsql::Error>((db, conn))
        };
        let (db, conn) = block_on_helper(&runtime, init)?;
        Ok(Self {
            inner: Arc::new(Inner { conn, db, runtime }),
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
        let now = unix_now();
        let conn = &self.inner.conn;
        let bytes_opt: Option<Vec<u8>> = self.inner.block_on(async {
            let mut rows = conn
                .query(
                    "SELECT payload FROM ca_entries WHERE hash = ?1 AND extr_ver = ?2 AND grammar = ?3",
                    params![hash, extr_ver, grammar],
                )
                .await?;
            let row = rows.next().await?;
            if let Some(row) = row {
                let bytes: Vec<u8> = row.get(0)?;
                // Touch last_used (best-effort — ignore errors)
                let _ = conn
                    .execute(
                        "UPDATE ca_entries SET last_used = ?1 WHERE hash = ?2 AND extr_ver = ?3 AND grammar = ?4",
                        params![now, hash, extr_ver, grammar],
                    )
                    .await;
                Ok::<_, libsql::Error>(Some(bytes))
            } else {
                Ok(None)
            }
        })?;
        if let Some(bytes) = bytes_opt {
            let value: T = bincode::deserialize(&bytes)?;
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
        let now = unix_now();
        let conn = &self.inner.conn;
        self.inner.block_on(async {
            conn.execute(
                "INSERT OR REPLACE INTO ca_entries (hash, extr_ver, grammar, payload, last_used)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![hash, extr_ver, grammar, bytes, now],
            )
            .await
        })?;
        Ok(())
    }

    /// Remove extraction entries for outdated extractor versions. Call once at startup.
    ///
    /// Only removes entries whose `extr_ver` does not start with `"symbols-"` (those
    /// belong to the symbol extraction cache and are managed separately). This lets
    /// symbol cache entries survive across index rebuilds.
    pub(crate) fn gc_stale_versions(&self, current_extr_ver: &str) -> Result<usize, Error> {
        let conn = &self.inner.conn;
        let n = self.inner.block_on(async {
            conn.execute(
                "DELETE FROM ca_entries WHERE extr_ver != ?1 AND extr_ver NOT LIKE 'symbols-%'",
                params![current_extr_ver],
            )
            .await
        })?;
        Ok(n as usize)
    }

    /// Remove symbol cache entries for outdated symbol cache versions. Call once at startup.
    ///
    /// Removes all `"symbols-*"` entries except those matching the current symbol
    /// cache version strings (`"symbols-v1-all"`, `"symbols-v1-public"`).
    pub(crate) fn gc_stale_symbol_versions(
        &self,
        current_versions: &[&str],
    ) -> Result<usize, Error> {
        // Build a NOT IN clause for the current versions
        let placeholders: String = current_versions
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "DELETE FROM ca_entries WHERE extr_ver LIKE 'symbols-%' AND extr_ver NOT IN ({placeholders})"
        );
        let owned: Vec<String> = current_versions.iter().map(|s| s.to_string()).collect();
        let conn = &self.inner.conn;
        let n = self.inner.block_on(async {
            // libsql accepts a Vec<Value> as IntoParams for variable-arity statements.
            let values: Vec<libsql::Value> = owned
                .iter()
                .map(|s| libsql::Value::Text(s.clone()))
                .collect();
            conn.execute(&sql, values).await
        })?;
        Ok(n as usize)
    }

    /// Evict oldest-accessed entries until DB file size is under `max_size_bytes`.
    /// Uses page_count * page_size as the size estimate; runs VACUUM after deletion.
    #[allow(dead_code)] // not yet wired into the refresh path; retained for future use
    pub(crate) fn evict_if_over_limit(&self) -> Result<(), Error> {
        let conn = &self.inner.conn;
        let max = self.max_size_bytes;
        self.inner.block_on(async {
            let size = current_db_size(conn).await.unwrap_or(0);
            if size <= max {
                return Ok::<_, libsql::Error>(());
            }
            let target = max * 9 / 10;
            loop {
                let current = current_db_size(conn).await.unwrap_or(0);
                if current <= target {
                    break;
                }
                let deleted = conn
                    .execute(
                        "DELETE FROM ca_entries WHERE (hash, extr_ver, grammar) IN (
                           SELECT hash, extr_ver, grammar FROM ca_entries ORDER BY last_used ASC LIMIT 100
                         )",
                        (),
                    )
                    .await?;
                if deleted == 0 {
                    break;
                }
            }
            conn.execute_batch("VACUUM;").await?;
            Ok(())
        })?;
        Ok(())
    }
}

async fn current_db_size(conn: &Connection) -> Result<u64, libsql::Error> {
    let mut rows = conn
        .query(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            (),
        )
        .await?;
    let n: i64 = if let Some(row) = rows.next().await? {
        row.get(0).unwrap_or(0)
    } else {
        0
    };
    Ok(n as u64)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Global singleton for the symbol extraction cache used by `Extractor`.
///
/// Initialized lazily on first access. Uses the same SQLite DB as the
/// content-addressed extraction cache. Returns `None` if the DB cannot be
/// opened (non-fatal — callers fall through to live parsing).
static SYMBOL_CACHE: OnceLock<Option<CaCache>> = OnceLock::new();

/// Current symbol cache version strings. Bump these when the `Symbol` struct or
/// post-processing logic changes in ways that invalidate cached results.
/// v2 (2026-07-15): `Symbol` gained a `complexity` field.
pub(crate) const SYMBOL_CACHE_VERSIONS: &[&str] = &["symbols-v2-all", "symbols-v2-public"];

/// Get the global symbol cache singleton.
///
/// Returns `None` if the cache could not be opened (e.g., no write permission
/// to `~/.config/normalize/`). Callers should treat `None` as a cache miss
/// and proceed with live parsing.
pub(crate) fn symbol_cache() -> Option<&'static CaCache> {
    SYMBOL_CACHE
        .get_or_init(|| {
            let path = CaCache::default_path();
            match CaCache::open(&path, 512 * 1024 * 1024) {
                Ok(cache) => {
                    // GC stale symbol cache entries at singleton init (best-effort).
                    if let Err(e) = cache.gc_stale_symbol_versions(SYMBOL_CACHE_VERSIONS) {
                        tracing::debug!("normalize-facts: symbol cache GC error: {}", e);
                    }
                    Some(cache)
                }
                Err(e) => {
                    tracing::debug!(
                        "normalize-facts: symbol cache unavailable at {}: {}",
                        path.display(),
                        e
                    );
                    None
                }
            }
        })
        .as_ref()
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
        // "old" is not a symbols- prefix entry, so it should be deleted
        let deleted = cache.gc_stale_versions("current").unwrap();
        assert_eq!(deleted, 1);
        let got: Option<Payload> = cache.get(hash.as_bytes(), "old", "rust").unwrap();
        assert_eq!(got, None);
        let got: Option<Payload> = cache.get(hash.as_bytes(), "current", "rust").unwrap();
        assert!(got.is_some());
    }

    #[test]
    fn gc_stale_versions_preserves_symbol_cache() {
        let cache = temp_cache();
        let hash = blake3::hash(b"test");
        let payload = Payload {
            symbols: vec![],
            count: 0,
        };
        // Put an extraction entry (old version) and a symbol cache entry
        cache.put(hash.as_bytes(), "old", "rust", &payload).unwrap();
        cache
            .put(hash.as_bytes(), "symbols-v1-all", "rust", &payload)
            .unwrap();
        // gc_stale_versions should delete "old" but NOT "symbols-v1-all"
        let deleted = cache.gc_stale_versions("current").unwrap();
        assert_eq!(deleted, 1);
        let got: Option<Payload> = cache.get(hash.as_bytes(), "old", "rust").unwrap();
        assert_eq!(got, None);
        let got: Option<Payload> = cache
            .get(hash.as_bytes(), "symbols-v1-all", "rust")
            .unwrap();
        assert!(
            got.is_some(),
            "symbol cache entries must survive extraction GC"
        );
    }

    #[test]
    fn gc_stale_symbol_versions() {
        let cache = temp_cache();
        let hash = blake3::hash(b"sym-test");
        let payload = Payload {
            symbols: vec![],
            count: 0,
        };
        cache
            .put(hash.as_bytes(), "symbols-v0-all", "rust", &payload)
            .unwrap();
        cache
            .put(hash.as_bytes(), "symbols-v1-all", "rust", &payload)
            .unwrap();
        cache
            .put(hash.as_bytes(), "symbols-v1-public", "rust", &payload)
            .unwrap();
        // Only v0 should be deleted
        let deleted = cache
            .gc_stale_symbol_versions(&["symbols-v1-all", "symbols-v1-public"])
            .unwrap();
        assert_eq!(deleted, 1);
        let got: Option<Payload> = cache
            .get(hash.as_bytes(), "symbols-v0-all", "rust")
            .unwrap();
        assert_eq!(got, None);
        let got: Option<Payload> = cache
            .get(hash.as_bytes(), "symbols-v1-all", "rust")
            .unwrap();
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
