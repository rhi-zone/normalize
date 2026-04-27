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
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS findings_cache (
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

    pub fn begin(&self) {
        self.conn.execute_batch("BEGIN;").ok();
    }

    pub fn commit(&self) {
        self.conn.execute_batch("COMMIT;").ok();
    }

    /// No-op — retained for API symmetry; callers should use begin/commit.
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

/// Trait for native rules that check individual files.
///
/// Implementing this trait gives automatic SQLite caching and parallel execution.
/// Rule authors implement `check_file()` and `to_diagnostics()` — the framework handles the rest.
pub trait FileRule: Send + Sync {
    /// Serializable per-file finding type.
    type Finding: serde::Serialize + serde::de::DeserializeOwned + Send;

    /// Unique engine name for cache keying (e.g. "long-function", "high-complexity").
    fn engine_name(&self) -> &str;

    /// Config hash for cache invalidation (e.g. threshold.to_string()).
    fn config_hash(&self) -> String;

    /// Check a single file. Returns findings for that file.
    /// `path` is absolute, `root` is the project root for computing relative paths.
    fn check_file(&self, path: &Path, root: &Path) -> Vec<Self::Finding>;

    /// Convert collected findings into a DiagnosticsReport.
    /// `findings` maps file path to that file's findings.
    /// `files_checked` is the total number of files examined (cached + fresh).
    fn to_diagnostics(
        &self,
        findings: Vec<(std::path::PathBuf, Vec<Self::Finding>)>,
        root: &Path,
        files_checked: usize,
    ) -> normalize_output::diagnostics::DiagnosticsReport;
}

/// Run a `FileRule` against a set of files with automatic caching and parallel execution.
///
/// 1. Walk files (or use `explicit_files`)
/// 2. Check cache for each file (sequential — fast DB lookups)
/// 3. Compute cache misses in parallel (rayon `par_iter`)
/// 4. Store new results in cache
/// 5. Merge cached + fresh findings and call `to_diagnostics()`
pub fn run_file_rule<R: FileRule>(
    rule: &R,
    root: &Path,
    explicit_files: Option<&[std::path::PathBuf]>,
    walk_config: &normalize_rules_config::WalkConfig,
) -> normalize_output::diagnostics::DiagnosticsReport {
    let files: Vec<std::path::PathBuf> = if let Some(ef) = explicit_files {
        ef.iter()
            .filter(|p| p.is_file())
            .filter(|p| normalize_languages::support_for_path(p).is_some())
            .cloned()
            .collect()
    } else {
        super::walk::gitignore_walk(root, walk_config)
            .filter(|e| e.path().is_file())
            .filter(|e| normalize_languages::support_for_path(e.path()).is_some())
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    let files_checked = files.len();
    let cache = FindingsCache::open(root);
    let config_hash = rule.config_hash();
    let engine = rule.engine_name();

    // Phase 1: separate cache hits from misses (sequential, fast DB lookups).
    let mut cached_findings: Vec<(std::path::PathBuf, Vec<R::Finding>)> = Vec::new();
    let mut cache_misses: Vec<std::path::PathBuf> = Vec::new();

    for file in &files {
        let path_key = file.to_string_lossy().to_string();
        let mtime = file_mtime_nanos(file);
        if mtime > 0
            && let Some(json) = cache.get(&path_key, mtime, &config_hash, engine)
            && let Ok(findings) = serde_json::from_str::<Vec<R::Finding>>(&json)
        {
            cached_findings.push((file.clone(), findings));
            continue;
        }
        cache_misses.push(file.clone());
    }

    // Phase 2: compute misses in parallel.
    use rayon::prelude::*;
    let fresh_findings: Vec<(std::path::PathBuf, Vec<R::Finding>)> = cache_misses
        .par_iter()
        .map(|path| {
            let findings = rule.check_file(path, root);
            (path.clone(), findings)
        })
        .collect();

    // Phase 3: store fresh results in cache (single transaction).
    cache.begin();
    for (path, findings) in &fresh_findings {
        let path_key = path.to_string_lossy().to_string();
        let mtime = file_mtime_nanos(path);
        if mtime > 0
            && let Ok(json) = serde_json::to_string(findings)
        {
            cache.put(&path_key, mtime, &config_hash, engine, &json);
        }
    }
    cache.commit();

    // Phase 4: merge and build report.
    let mut all_findings: Vec<(std::path::PathBuf, Vec<R::Finding>)> = cached_findings;
    all_findings.extend(fresh_findings);

    rule.to_diagnostics(all_findings, root, files_checked)
}
