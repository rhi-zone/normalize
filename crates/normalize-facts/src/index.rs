use crate::symbols::SymbolParser;
use ignore::WalkBuilder;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use libsql::{Connection, Database, params};
pub use normalize_facts_core::IndexedFile;
use normalize_facts_core::{FlatImport, FlatSymbol, TypeRef};
use normalize_languages::support_for_path;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A parsed symbol ready for database insertion.
struct ParsedSymbol {
    name: String,
    kind: String,
    start_line: usize,
    end_line: usize,
    parent: Option<String>,
    visibility: String,
    attributes: Vec<String>,
    is_interface_impl: bool,
    implements: Vec<String>,
    docstring: Option<String>,
}

/// One call-site entry: (caller_symbol, callee_name, callee_qualifier, access, line).
type CallEntry = (String, String, Option<String>, Option<String>, usize);

/// Parsed data for a single file, ready for database insertion
struct ParsedFileData {
    file_path: String,
    symbols: Vec<ParsedSymbol>,
    calls: Vec<CallEntry>,
    /// imports (for Python files only)
    imports: Vec<FlatImport>,
    /// (type_name, method_name) for interface/class method signatures
    type_methods: Vec<(String, String)>,
    /// Type-to-type references (field types, param types, extends, etc.)
    type_refs: Vec<TypeRef>,
}

// Not yet public - just delete .normalize/index.sqlite on schema changes
const SCHEMA_VERSION: i64 = 8;

/// Check if a file path has a supported source extension.
fn is_source_file(path: &str) -> bool {
    normalize_languages::support_for_path(std::path::Path::new(path)).is_some()
}

/// Generate SQL WHERE clause for filtering source files.
/// Returns: "path LIKE '%.py' OR path LIKE '%.rs' OR ..."
fn source_extensions_sql_filter() -> String {
    let mut extensions: Vec<&str> = normalize_languages::supported_languages()
        .iter()
        .flat_map(|lang| lang.extensions().iter().copied())
        .collect();
    extensions.sort_unstable();
    extensions.dedup();
    extensions
        .iter()
        .map(|ext| format!("path LIKE '%.{}'", ext))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Result from symbol search
#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolMatch {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub parent: Option<String>,
}

/// Files that changed since last index
#[derive(Debug, Default)]
pub struct ChangedFiles {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

/// Call graph statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct CallGraphStats {
    pub symbols: usize,
    pub calls: usize,
    pub imports: usize,
}

pub struct FileIndex {
    conn: Connection,
    #[allow(dead_code)]
    db: Database,
    root: PathBuf,
    progress: bool,
}

impl FileIndex {
    /// Open or create an index at the specified database path.
    /// On corruption, automatically deletes and recreates the index.
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    /// * `root` - Project root directory (used for file walking during refresh)
    pub async fn open(db_path: &Path, root: &Path) -> Result<Self, libsql::Error> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::warn!(
                "normalize-facts: failed to create index directory {:?}: {}",
                parent,
                e
            );
        }

        // Try to open, with recovery on corruption
        match Self::try_open(db_path, root).await {
            Ok(idx) => Ok(idx),
            Err(e) => {
                // Check for corruption-like errors
                let err_str = e.to_string().to_lowercase();
                let is_corruption = err_str.contains("corrupt")
                    || err_str.contains("malformed")
                    || err_str.contains("disk i/o error")
                    || err_str.contains("not a database")
                    || err_str.contains("database disk image")
                    || err_str.contains("integrity check failed");

                if is_corruption {
                    tracing::warn!("Index corrupted, rebuilding: {}", e);
                    // Delete corrupted database and retry
                    let _ = std::fs::remove_file(db_path);
                    // Also remove journal/wal files if they exist
                    let _ = std::fs::remove_file(db_path.with_extension("sqlite-journal"));
                    let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
                    let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
                    Self::try_open(db_path, root).await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Internal: try to open database without recovery
    async fn try_open(db_path: &Path, root: &Path) -> Result<Self, libsql::Error> {
        let db = libsql::Builder::new_local(db_path).build().await?;
        let conn = db.connect()?;

        // Quick integrity check - this will catch most corruption
        // PRAGMA quick_check is faster than full integrity_check
        let mut rows = conn.query("PRAGMA quick_check(1)", ()).await?;
        let integrity: String = if let Some(row) = rows.next().await? {
            row.get(0).unwrap_or_else(|_| "error".to_string())
        } else {
            "error".to_string()
        };
        if integrity != "ok" {
            return Err(libsql::Error::SqliteFailure(
                11, // SQLITE_CORRUPT
                format!("Database integrity check failed: {}", integrity),
            ));
        }

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                is_dir INTEGER NOT NULL,
                mtime INTEGER NOT NULL,
                lines INTEGER NOT NULL DEFAULT 0
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_name ON files(path)",
            (),
        )
        .await?;

        // Call graph for fast caller/callee lookups
        conn.execute(
            "CREATE TABLE IF NOT EXISTS calls (
                caller_file TEXT NOT NULL,
                caller_symbol TEXT NOT NULL,
                callee_name TEXT NOT NULL,
                callee_qualifier TEXT,
                callee_resolved_file TEXT,
                line INTEGER NOT NULL,
                access TEXT
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_calls_callee ON calls(callee_name)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_calls_caller ON calls(caller_file, caller_symbol)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_calls_qualifier ON calls(callee_qualifier)",
            (),
        )
        .await?;
        // May fail on old DBs where the column doesn't exist yet; migration below adds it.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_calls_resolved ON calls(callee_resolved_file)",
            (),
        )
        .await
        .ok();

        // Symbol definitions
        conn.execute(
            "CREATE TABLE IF NOT EXISTS symbols (
                file TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                parent TEXT,
                visibility TEXT NOT NULL DEFAULT 'public',
                is_impl INTEGER NOT NULL DEFAULT 0
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file)",
            (),
        )
        .await?;

        // Symbol attributes (one row per attribute per symbol)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS symbol_attributes (
                file TEXT NOT NULL,
                name TEXT NOT NULL,
                attribute TEXT NOT NULL
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_symbol_attributes_file_name ON symbol_attributes(file, name)",
            (),
        )
        .await?;

        // Symbol implements (one row per interface/trait per symbol)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS symbol_implements (
                file TEXT NOT NULL,
                name TEXT NOT NULL,
                interface TEXT NOT NULL
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_symbol_implements_file_name ON symbol_implements(file, name)",
            (),
        )
        .await?;

        // Import tracking
        conn.execute(
            "CREATE TABLE IF NOT EXISTS imports (
                file TEXT NOT NULL,
                module TEXT,
                name TEXT NOT NULL,
                alias TEXT,
                line INTEGER NOT NULL,
                resolved_file TEXT
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_imports_name ON imports(name)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_imports_module ON imports(module)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_imports_resolved ON imports(resolved_file)",
            (),
        )
        .await?;

        // Type method signatures
        conn.execute(
            "CREATE TABLE IF NOT EXISTS type_methods (
                file TEXT NOT NULL,
                type_name TEXT NOT NULL,
                method_name TEXT NOT NULL,
                PRIMARY KEY (file, type_name, method_name)
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_type_methods_type ON type_methods(type_name)",
            (),
        )
        .await?;

        // Type references (type-to-type dependencies)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS type_refs (
                file TEXT NOT NULL,
                source_symbol TEXT NOT NULL,
                target_type TEXT NOT NULL,
                kind TEXT NOT NULL,
                line INTEGER NOT NULL
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_type_refs_file ON type_refs(file)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_type_refs_source ON type_refs(source_symbol)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_type_refs_target ON type_refs(target_type)",
            (),
        )
        .await?;

        // Migrate existing tables: add columns that may be missing from older schemas.
        // SQLite errors on duplicate ADD COLUMN, so we ignore failures.
        conn.execute(
            "ALTER TABLE symbols ADD COLUMN visibility TEXT NOT NULL DEFAULT 'public'",
            (),
        )
        .await
        .ok();
        conn.execute(
            "ALTER TABLE symbols ADD COLUMN is_impl INTEGER NOT NULL DEFAULT 0",
            (),
        )
        .await
        .ok();
        // resolved_file was added to imports after schema version 5 was already set;
        // run unconditionally so existing v5 DBs without the column get migrated.
        conn.execute("ALTER TABLE imports ADD COLUMN resolved_file TEXT", ())
            .await
            .ok();

        // Check schema version
        let mut rows = conn
            .query(
                "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'schema_version'",
                (),
            )
            .await?;
        let version: i64 = if let Some(row) = rows.next().await? {
            row.get(0).unwrap_or(0)
        } else {
            0
        };

        if version != SCHEMA_VERSION {
            // Reset on schema change
            conn.execute("DELETE FROM files", ()).await?;
            conn.execute("DELETE FROM calls", ()).await?;
            conn.execute("DELETE FROM symbols", ()).await?;
            conn.execute("DELETE FROM imports", ()).await?;
            // Add new columns that may not exist in older schema versions.
            // Use .ok() to tolerate "duplicate column" errors on already-migrated DBs.
            conn.execute("ALTER TABLE imports ADD COLUMN resolved_file TEXT", ())
                .await
                .ok(); // ignore "duplicate column" error on fresh DBs
            conn.execute("ALTER TABLE calls ADD COLUMN callee_resolved_file TEXT", ())
                .await
                .ok(); // ignore "duplicate column" error on fresh DBs
            conn.execute("ALTER TABLE calls ADD COLUMN access TEXT", ())
                .await
                .ok(); // ignore "duplicate column" error on fresh DBs
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_calls_resolved ON calls(callee_resolved_file)",
                (),
            )
            .await?;
            conn.execute("DELETE FROM type_methods", ()).await?;
            conn.execute("DELETE FROM type_refs", ()).await?;
            conn.execute("DELETE FROM symbol_attributes", ()).await?;
            conn.execute("DELETE FROM symbol_implements", ()).await?;
            // co_change_edges: clear on schema bump so the next rebuild repopulates.
            conn.execute("DELETE FROM co_change_edges", ()).await.ok();
            conn.execute("DELETE FROM meta WHERE key = 'co_change_last_commit'", ())
                .await
                .ok();
            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
                params![SCHEMA_VERSION.to_string()],
            )
            .await?;
        }

        // Create convenience views for agent queries.
        // These are idempotent (CREATE VIEW IF NOT EXISTS) and safe to run on every open.

        // entry_points: public symbols that are never called internally.
        // Identifies API surface that external callers enter through — functions/types
        // that are exported but have no recorded callers within the indexed codebase.
        // Useful for finding dead public API candidates and top-level entry symbols.
        conn.execute(
            "CREATE VIEW IF NOT EXISTS entry_points AS
             SELECT s.file, s.name, s.kind, s.start_line, s.end_line
             FROM symbols s
             WHERE s.visibility = 'public'
               AND NOT EXISTS (
                   SELECT 1 FROM calls c WHERE c.callee_name = s.name
               )",
            (),
        )
        .await
        .ok();

        // external_deps: imports whose module specifier could not be resolved to a
        // file within the indexed root (resolved_file IS NULL). These represent
        // third-party packages, stdlib imports, or imports outside the project root.
        // Used to distinguish in-project edges from external dependencies in analysis.
        conn.execute(
            "CREATE VIEW IF NOT EXISTS external_deps AS
             SELECT file, module, name, alias, line
             FROM imports
             WHERE resolved_file IS NULL",
            (),
        )
        .await
        .ok();

        // external_surface: public symbols that are called by files whose own imports
        // include at least one unresolved (external) dependency.
        // Identifies the boundary between internal implementation and externally-facing
        // API — the symbols that external-dependency-using files actually invoke.
        conn.execute(
            "CREATE VIEW IF NOT EXISTS external_surface AS
             SELECT DISTINCT s.file, s.name, s.kind, s.start_line, s.end_line
             FROM symbols s
             WHERE s.visibility = 'public'
               AND EXISTS (
                   SELECT 1 FROM calls c
                   WHERE c.callee_name = s.name
                     AND EXISTS (
                         SELECT 1 FROM external_deps ed WHERE ed.file = c.caller_file
                     )
               )",
            (),
        )
        .await
        .ok();

        // Co-change edges: file pairs that appear together in commits.
        // Populated by rebuild_co_change_edges(); queried by coupling-clusters.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS co_change_edges (
                file_a TEXT NOT NULL,
                file_b TEXT NOT NULL,
                count INTEGER NOT NULL,
                PRIMARY KEY (file_a, file_b)
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_co_change_file_a ON co_change_edges(file_a)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_co_change_file_b ON co_change_edges(file_b)",
            (),
        )
        .await?;

        // Daemon diagnostics cache: persisted issue blobs, one row per engine.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daemon_diagnostics (
                engine TEXT PRIMARY KEY,
                issues_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            (),
        )
        .await?;

        Ok(Self {
            conn,
            db,
            root: root.to_path_buf(),
            progress: false,
        })
    }

    /// Enable progress bar output for long-running operations (refresh, call graph).
    /// Only shows bars when stderr is a terminal.
    pub fn set_progress(&mut self, enabled: bool) {
        self.progress = enabled;
    }

    /// Get a reference to the underlying SQLite connection for direct queries
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get files that have changed since last index
    pub async fn get_changed_files(&self) -> Result<ChangedFiles, libsql::Error> {
        let mut result = ChangedFiles::default();

        // Get all indexed files with their mtimes
        let mut indexed: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        {
            let mut rows = self
                .conn
                .query("SELECT path, mtime FROM files WHERE is_dir = 0", ())
                .await?;
            while let Some(row) = rows.next().await? {
                let path: String = row.get(0)?;
                let mtime: i64 = row.get(1)?;
                indexed.insert(path, mtime);
            }
        }

        // Walk current filesystem
        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        let mut seen = std::collections::HashSet::new();
        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(&self.root) {
                let rel_str = rel.to_string_lossy().to_string();
                // Skip internal directories
                if rel_str.is_empty() || rel_str == ".git" || rel_str.starts_with(".git/") {
                    continue;
                }
                seen.insert(rel_str.clone());

                let current_mtime = path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                if let Some(&indexed_mtime) = indexed.get(&rel_str) {
                    if current_mtime > indexed_mtime {
                        result.modified.push(rel_str);
                    }
                } else {
                    result.added.push(rel_str);
                }
            }
        }

        // Find deleted files
        for path in indexed.keys() {
            if !seen.contains(path) {
                result.deleted.push(path.clone());
            }
        }

        Ok(result)
    }

    /// Check if refresh is needed using fast heuristics.
    /// Returns true if changes are likely.
    async fn needs_refresh(&self) -> bool {
        let mut rows = match self
            .conn
            .query(
                "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'last_indexed'",
                (),
            )
            .await
        {
            Ok(r) => r,
            Err(_) => return true,
        };
        let last_indexed: i64 = match rows.next().await {
            Ok(Some(row)) => row.get(0).unwrap_or(0),
            _ => 0,
        };

        // Never indexed
        if last_indexed == 0 {
            return true;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Allow 60s staleness - don't check on every call
        if now - last_indexed < 60 {
            return false;
        }

        // Check mtimes of top-level entries (catches new/deleted files)
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') {
                    continue;
                }
                if let Ok(meta) = entry.metadata()
                    && let Ok(mtime) = meta.modified()
                {
                    let mtime_secs = mtime
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    if mtime_secs > last_indexed {
                        return true;
                    }
                }
            }
        }

        // Sample some indexed files to catch modifications
        // Check ~100 files spread across the index
        if let Ok(mut rows) = self
            .conn
            .query(
                "SELECT path, mtime FROM files WHERE is_dir = 0 ORDER BY RANDOM() LIMIT 100",
                (),
            )
            .await
        {
            while let Ok(Some(row)) = rows.next().await {
                let path: String = match row.get(0) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let indexed_mtime: i64 = match row.get(1) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let full_path = self.root.join(&path);
                if let Ok(meta) = full_path.metadata()
                    && let Ok(mtime) = meta.modified()
                {
                    let current_mtime = mtime
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    if current_mtime > indexed_mtime {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Refresh only files that have changed (faster than full refresh).
    /// Returns the list of changed file paths (absolute) that were added, modified, or deleted.
    /// The count can be derived from `.len()`.
    pub async fn incremental_refresh(&mut self) -> Result<Vec<PathBuf>, libsql::Error> {
        if !self.needs_refresh().await {
            return Ok(Vec::new());
        }

        let changed = self.get_changed_files().await?;
        let total_changes = changed.added.len() + changed.modified.len() + changed.deleted.len();

        if total_changes == 0 {
            return Ok(Vec::new());
        }

        self.conn.execute("BEGIN", ()).await?;

        // Delete removed files
        for path in &changed.deleted {
            self.conn
                .execute("DELETE FROM files WHERE path = ?1", params![path.clone()])
                .await?;
        }

        // Update/insert changed files
        for path in changed.added.iter().chain(changed.modified.iter()) {
            let full_path = self.root.join(path);
            let is_dir = full_path.is_dir();
            let mtime = full_path
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            // Count lines for text files (binary files will fail read_to_string and get 0)
            let lines = if is_dir {
                0
            } else {
                std::fs::read_to_string(&full_path)
                    .map(|s| s.lines().count())
                    .unwrap_or(0)
            };

            self.conn.execute(
                "INSERT OR REPLACE INTO files (path, is_dir, mtime, lines) VALUES (?1, ?2, ?3, ?4)",
                params![path.clone(), is_dir as i64, mtime, lines as i64],
            ).await?;
        }

        // Update last indexed time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('last_indexed', ?1)",
                params![now.to_string()],
            )
            .await?;

        self.conn.execute("COMMIT", ()).await?;

        // Collect all changed paths as absolute PathBufs
        let all_changed: Vec<PathBuf> = changed
            .added
            .iter()
            .chain(changed.modified.iter())
            .chain(changed.deleted.iter())
            .map(|p| self.root.join(p))
            .collect();

        Ok(all_changed)
    }

    /// Execute a raw SQL statement (for maintenance operations).
    pub async fn execute(&self, sql: &str) -> Result<u64, libsql::Error> {
        self.conn.execute(sql, ()).await
    }

    /// Run an arbitrary read-only SQL query and return results as a list of row maps.
    ///
    /// Each row is a `serde_json::Map` from column name to value.
    /// Useful for agent-driven exploration of the structural index.
    pub async fn raw_query(
        &self,
        sql: &str,
    ) -> Result<Vec<serde_json::Map<String, serde_json::Value>>, libsql::Error> {
        let mut rows = self.conn.query(sql, ()).await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            let col_count = row.column_count();
            let mut map = serde_json::Map::new();
            for i in 0..col_count {
                let col_name = row.column_name(i).unwrap_or("?").to_string();
                let value = match row.get_value(i)? {
                    libsql::Value::Null => serde_json::Value::Null,
                    libsql::Value::Integer(n) => serde_json::Value::Number(n.into()),
                    libsql::Value::Real(f) => serde_json::json!(f),
                    libsql::Value::Text(s) => serde_json::Value::String(s),
                    libsql::Value::Blob(b) => {
                        serde_json::Value::String(format!("<blob {} bytes>", b.len()))
                    }
                };
                map.insert(col_name, value);
            }
            result.push(map);
        }
        Ok(result)
    }

    /// Refresh the index by walking the filesystem
    pub async fn refresh(&mut self) -> Result<usize, libsql::Error> {
        let walker = WalkBuilder::new(&self.root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        self.conn.execute("BEGIN", ()).await?;

        // Clear existing files
        self.conn.execute("DELETE FROM files", ()).await?;

        let pb = if self.progress && std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template("{spinner:.cyan} {msg} [{elapsed_precise}]")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.set_message("Scanning files...");
            pb
        } else {
            ProgressBar::hidden()
        };

        let mut count = 0;
        for entry in walker.flatten() {
            let path = entry.path();
            if let Ok(rel) = path.strip_prefix(&self.root) {
                let rel_str = rel.to_string_lossy().to_string();
                // Skip internal directories
                if rel_str.is_empty() || rel_str == ".git" || rel_str.starts_with(".git/") {
                    continue;
                }

                let is_dir = path.is_dir();
                let mtime = path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                // Count lines for text files (binary files will fail read_to_string and get 0)
                let lines = if is_dir {
                    0
                } else {
                    std::fs::read_to_string(path)
                        .map(|s| s.lines().count())
                        .unwrap_or(0)
                };

                self.conn
                    .execute(
                        "INSERT INTO files (path, is_dir, mtime, lines) VALUES (?1, ?2, ?3, ?4)",
                        params![rel_str, is_dir as i64, mtime, lines as i64],
                    )
                    .await?;
                count += 1;
                pb.set_message(format!("Scanning files... {count}"));
                pb.tick();
            }
        }

        pb.finish_and_clear();

        // Update last indexed time
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('last_indexed', ?1)",
                params![now.to_string()],
            )
            .await?;

        self.conn.execute("COMMIT", ()).await?;

        Ok(count)
    }

    /// Get all files from the index
    pub async fn all_files(&self) -> Result<Vec<IndexedFile>, libsql::Error> {
        let mut rows = self
            .conn
            .query("SELECT path, is_dir, mtime, lines FROM files", ())
            .await?;
        let mut files = Vec::new();
        while let Some(row) = rows.next().await? {
            files.push(IndexedFile {
                path: row.get(0)?,
                is_dir: row.get::<i64>(1)? != 0,
                mtime: row.get(2)?,
                lines: u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
            });
        }
        Ok(files)
    }

    /// Search files by exact name match
    pub async fn find_by_name(&self, name: &str) -> Result<Vec<IndexedFile>, libsql::Error> {
        let pattern = format!("%/{}", name);
        let mut rows = self
            .conn
            .query(
                "SELECT path, is_dir, mtime, lines FROM files WHERE path LIKE ?1 OR path = ?2",
                params![pattern, name],
            )
            .await?;
        let mut files = Vec::new();
        while let Some(row) = rows.next().await? {
            files.push(IndexedFile {
                path: row.get(0)?,
                is_dir: row.get::<i64>(1)? != 0,
                mtime: row.get(2)?,
                lines: u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
            });
        }
        Ok(files)
    }

    /// Search files by stem (filename without extension)
    pub async fn find_by_stem(&self, stem: &str) -> Result<Vec<IndexedFile>, libsql::Error> {
        let pattern = format!("%/{}%", stem);
        let mut rows = self
            .conn
            .query(
                "SELECT path, is_dir, mtime, lines FROM files WHERE path LIKE ?1",
                params![pattern],
            )
            .await?;
        let mut files = Vec::new();
        while let Some(row) = rows.next().await? {
            files.push(IndexedFile {
                path: row.get(0)?,
                is_dir: row.get::<i64>(1)? != 0,
                mtime: row.get(2)?,
                lines: u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
            });
        }
        Ok(files)
    }

    /// Count indexed files
    pub async fn count(&self) -> Result<usize, libsql::Error> {
        let mut rows = self.conn.query("SELECT COUNT(*) FROM files", ()).await?;
        if let Some(row) = rows.next().await? {
            Ok(u64::try_from(row.get::<i64>(0)?).unwrap_or(0) as usize)
        } else {
            Ok(0)
        }
    }

    /// Index symbols and call graph for a file
    #[allow(dead_code)] // FileIndex API - used by daemon
    pub async fn index_file_symbols(
        &self,
        path: &str,
        symbols: &[FlatSymbol],
        calls: &[(String, String, usize)],
    ) -> Result<(), libsql::Error> {
        // Insert symbols
        for sym in symbols {
            self.conn.execute(
                "INSERT INTO symbols (file, name, kind, start_line, end_line, parent, visibility, is_impl) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![path.to_string(), sym.name.clone(), sym.kind.as_str(), sym.start_line as i64, sym.end_line as i64, sym.parent.clone(), sym.visibility.as_str(), sym.is_interface_impl as i64],
            ).await?;
            for attr in &sym.attributes {
                self.conn
                    .execute(
                        "INSERT INTO symbol_attributes (file, name, attribute) VALUES (?1, ?2, ?3)",
                        params![path.to_string(), sym.name.clone(), attr.clone()],
                    )
                    .await?;
            }
            if let Some(doc) = &sym.docstring {
                self.conn
                    .execute(
                        "INSERT INTO symbol_attributes (file, name, attribute) VALUES (?1, ?2, ?3)",
                        params![path.to_string(), sym.name.clone(), format!("doc:{doc}")],
                    )
                    .await?;
            }
            for iface in &sym.implements {
                self.conn
                    .execute(
                        "INSERT INTO symbol_implements (file, name, interface) VALUES (?1, ?2, ?3)",
                        params![path.to_string(), sym.name.clone(), iface.clone()],
                    )
                    .await?;
            }
        }

        // Insert calls (caller_symbol, callee_name, line)
        for (caller_symbol, callee_name, line) in calls {
            self.conn.execute(
                "INSERT INTO calls (caller_file, caller_symbol, callee_name, line) VALUES (?1, ?2, ?3, ?4)",
                params![path.to_string(), caller_symbol.clone(), callee_name.clone(), *line as i64],
            ).await?;
        }

        Ok(())
    }

    /// Find callers of a specific symbol definition (from call graph).
    ///
    /// `def_file` is the file that contains the definition being searched. Results are
    /// restricted to files that are `def_file` itself (self-recursive calls) or that
    /// explicitly import the symbol. This prevents false positives from unrelated
    /// functions with the same name in other modules.
    ///
    /// Resolves through imports: if file A imports X as Y and calls Y(), it is found
    /// as a caller of X. Also handles qualified calls (`foo.bar()`) and `self.method()`
    /// resolved to the containing class.
    pub async fn find_callers(
        &self,
        symbol_name: &str,
        def_file: &str,
    ) -> Result<Vec<(String, String, usize, Option<String>)>, libsql::Error> {
        // Handle Class.method format - split and search for method within class
        let (class_filter, method_name) = if symbol_name.contains('.') {
            let parts: Vec<&str> = symbol_name.splitn(2, '.').collect();
            (Some(parts[0]), parts[1])
        } else {
            (None, symbol_name)
        };

        // If searching for Class.method, find callers that call self.method within that class
        if let Some(class_name) = class_filter {
            let mut rows = self
                .conn
                .query(
                    "SELECT c.caller_file, c.caller_symbol, c.line, c.access
                 FROM calls c
                 JOIN symbols s ON c.caller_file = s.file AND c.caller_symbol = s.name
                 WHERE c.callee_name = ?1 AND c.callee_qualifier = 'self' AND s.parent = ?2",
                    params![method_name, class_name],
                )
                .await?;
            let mut callers = Vec::new();
            while let Some(row) = rows.next().await? {
                callers.push((
                    row.get(0)?,
                    row.get(1)?,
                    u64::try_from(row.get::<i64>(2)?).unwrap_or(0) as usize,
                    row.get::<Option<String>>(3)?,
                ));
            }

            if !callers.is_empty() {
                return Ok(callers);
            }
        }

        // Use callee_resolved_file when available for precise call resolution.
        // Falls back to import-based matching when callee_resolved_file is NULL
        // (external packages, unresolved modules).
        //
        // Branch 1: callee_resolved_file = def_file (precise match)
        // Branch 2: Same-file calls (caller_file = def_file, no qualifier)
        // Branch 3: Import-based fallback for unresolved calls (callee_resolved_file IS NULL)
        // Branch 4: self.method() calls within a class
        let mut rows = self.conn.query(
            "SELECT caller_file, caller_symbol, line, access FROM calls
             WHERE callee_name = ?1 AND callee_resolved_file = ?2
             UNION
             SELECT caller_file, caller_symbol, line, access FROM calls
             WHERE callee_name = ?1 AND caller_file = ?2
               AND callee_resolved_file IS NULL AND callee_qualifier IS NULL
             UNION
             SELECT c.caller_file, c.caller_symbol, c.line, c.access
             FROM calls c
             JOIN imports i ON c.caller_file = i.file AND c.callee_name = COALESCE(i.alias, i.name)
             WHERE i.name = ?1 AND c.callee_resolved_file IS NULL
               AND (i.resolved_file = ?2 OR i.resolved_file IS NULL)
             UNION
             SELECT c.caller_file, c.caller_symbol, c.line, c.access
             FROM calls c
             JOIN imports i ON c.caller_file = i.file AND c.callee_qualifier = COALESCE(i.alias, i.name)
             WHERE c.callee_name = ?1 AND i.module IS NULL AND c.callee_resolved_file IS NULL
               AND (i.resolved_file = ?2 OR i.resolved_file IS NULL)
             UNION
             SELECT c.caller_file, c.caller_symbol, c.line, c.access
             FROM calls c
             JOIN symbols s ON c.caller_file = s.file AND c.caller_symbol = s.name
             WHERE c.callee_name = ?1 AND c.callee_qualifier = 'self'
               AND s.parent IS NOT NULL AND c.callee_resolved_file IS NULL",
            params![method_name, def_file],
        ).await?;
        let mut callers = Vec::new();
        while let Some(row) = rows.next().await? {
            callers.push((
                row.get(0)?,
                row.get(1)?,
                u64::try_from(row.get::<i64>(2)?).unwrap_or(0) as usize,
                row.get::<Option<String>>(3)?,
            ));
        }

        Ok(callers)
    }

    /// Find callees of a symbol (what it calls)
    pub async fn find_callees(
        &self,
        file: &str,
        symbol_name: &str,
    ) -> Result<Vec<(String, usize, Option<String>)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT callee_name, line, access FROM calls WHERE caller_file = ?1 AND caller_symbol = ?2",
                params![file, symbol_name],
            )
            .await?;
        let mut callees = Vec::new();
        while let Some(row) = rows.next().await? {
            callees.push((
                row.get(0)?,
                u64::try_from(row.get::<i64>(1)?).unwrap_or(0) as usize,
                row.get::<Option<String>>(2)?,
            ));
        }
        Ok(callees)
    }

    /// Find callees with their resolved definition file.
    ///
    /// Returns `(callee_name, line, Option<def_file>)` where `def_file` is the
    /// root-relative path of the file that defines the callee, resolved via the
    /// imports table's `resolved_file` column. `None` means the callee is locally
    /// defined, external, or could not be resolved.
    pub async fn find_callees_resolved(
        &self,
        file: &str,
        symbol_name: &str,
    ) -> Result<Vec<(String, usize, Option<String>)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT c.callee_name, c.line, i.resolved_file
                 FROM calls c
                 LEFT JOIN imports i
                   ON c.caller_file = i.file
                   AND c.callee_name = COALESCE(i.alias, i.name)
                 WHERE c.caller_file = ?1 AND c.caller_symbol = ?2",
                params![file, symbol_name],
            )
            .await?;
        let mut callees = Vec::new();
        while let Some(row) = rows.next().await? {
            callees.push((
                row.get(0)?,
                u64::try_from(row.get::<i64>(1)?).unwrap_or(0) as usize,
                row.get::<Option<String>>(2)?,
            ));
        }
        Ok(callees)
    }

    /// Find a symbol by name
    pub async fn find_symbol(
        &self,
        name: &str,
    ) -> Result<Vec<(String, String, usize, usize)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, kind, start_line, end_line FROM symbols WHERE name = ?1",
                params![name],
            )
            .await?;
        let mut symbols = Vec::new();
        while let Some(row) = rows.next().await? {
            symbols.push((
                row.get(0)?,
                row.get(1)?,
                u64::try_from(row.get::<i64>(2)?).unwrap_or(0) as usize,
                u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
            ));
        }
        Ok(symbols)
    }

    /// Get all distinct symbol names as a HashSet.
    pub async fn all_symbol_names(
        &self,
    ) -> Result<std::collections::HashSet<String>, libsql::Error> {
        let mut rows = self
            .conn
            .query("SELECT DISTINCT name FROM symbols", ())
            .await?;
        let mut names = std::collections::HashSet::new();
        while let Some(row) = rows.next().await? {
            names.insert(row.get(0)?);
        }
        Ok(names)
    }

    /// Find symbols by name with fuzzy matching, optional kind filter, and limit
    pub async fn find_symbols(
        &self,
        query: &str,
        kind: Option<&str>,
        fuzzy: bool,
        limit: usize,
    ) -> Result<Vec<SymbolMatch>, libsql::Error> {
        let query_lower = query.to_lowercase();
        let prefix_pattern = format!("{}%", query_lower);
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);

        let mut symbols = Vec::new();

        if fuzzy {
            let pattern = format!("%{}%", query_lower);
            let mut rows = if let Some(k) = kind {
                self.conn
                    .query(
                        "SELECT name, kind, file, start_line, end_line, parent FROM symbols
                     WHERE LOWER(name) LIKE ?1 AND kind = ?2
                     ORDER BY
                       CASE WHEN LOWER(name) = ?3 THEN 0
                            WHEN LOWER(name) LIKE ?4 THEN 1
                            ELSE 2 END,
                       LENGTH(name), name
                     LIMIT ?5",
                        params![pattern, k, query_lower, prefix_pattern, limit_i64],
                    )
                    .await?
            } else {
                self.conn
                    .query(
                        "SELECT name, kind, file, start_line, end_line, parent FROM symbols
                     WHERE LOWER(name) LIKE ?1
                     ORDER BY
                       CASE WHEN LOWER(name) = ?2 THEN 0
                            WHEN LOWER(name) LIKE ?3 THEN 1
                            ELSE 2 END,
                       LENGTH(name), name
                     LIMIT ?4",
                        params![pattern, query_lower, prefix_pattern, limit_i64],
                    )
                    .await?
            };

            while let Some(row) = rows.next().await? {
                symbols.push(SymbolMatch {
                    name: row.get(0)?,
                    kind: row.get(1)?,
                    file: row.get(2)?,
                    start_line: u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
                    end_line: u64::try_from(row.get::<i64>(4)?).unwrap_or(0) as usize,
                    parent: row.get(5)?,
                });
            }
        } else {
            // Exact match
            let mut rows = if let Some(k) = kind {
                self.conn
                    .query(
                        "SELECT name, kind, file, start_line, end_line, parent FROM symbols
                     WHERE LOWER(name) = LOWER(?1) AND kind = ?2
                     LIMIT ?3",
                        params![query, k, limit_i64],
                    )
                    .await?
            } else {
                self.conn
                    .query(
                        "SELECT name, kind, file, start_line, end_line, parent FROM symbols
                     WHERE LOWER(name) = LOWER(?1)
                     LIMIT ?2",
                        params![query, limit_i64],
                    )
                    .await?
            };

            while let Some(row) = rows.next().await? {
                symbols.push(SymbolMatch {
                    name: row.get(0)?,
                    kind: row.get(1)?,
                    file: row.get(2)?,
                    start_line: u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
                    end_line: u64::try_from(row.get::<i64>(4)?).unwrap_or(0) as usize,
                    parent: row.get(5)?,
                });
            }
        }

        Ok(symbols)
    }

    /// Get call graph stats
    pub async fn call_graph_stats(&self) -> Result<CallGraphStats, libsql::Error> {
        let symbols = {
            let mut rows = self.conn.query("SELECT COUNT(*) FROM symbols", ()).await?;
            if let Some(row) = rows.next().await? {
                u64::try_from(row.get::<i64>(0)?).unwrap_or(0) as usize
            } else {
                0
            }
        };
        let calls = {
            let mut rows = self.conn.query("SELECT COUNT(*) FROM calls", ()).await?;
            if let Some(row) = rows.next().await? {
                u64::try_from(row.get::<i64>(0)?).unwrap_or(0) as usize
            } else {
                0
            }
        };
        let imports = {
            let mut rows = self.conn.query("SELECT COUNT(*) FROM imports", ()).await?;
            if let Some(row) = rows.next().await? {
                u64::try_from(row.get::<i64>(0)?).unwrap_or(0) as usize
            } else {
                0
            }
        };
        Ok(CallGraphStats {
            symbols,
            calls,
            imports,
        })
    }

    /// Load all call edges from the calls table.
    /// Returns Vec<(caller_file, caller_symbol, callee_name)>.
    /// Used by test-gaps analysis for bulk caller lookup.
    pub async fn all_call_edges(&self) -> Result<Vec<(String, String, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT caller_file, caller_symbol, callee_name FROM calls",
                (),
            )
            .await?;
        let mut edges = Vec::new();
        while let Some(row) = rows.next().await? {
            edges.push((row.get(0)?, row.get(1)?, row.get(2)?));
        }
        Ok(edges)
    }

    /// Load all imports from the imports table.
    /// Returns Vec<(file, module, name, line)>.
    /// Used by rules for building relations.
    pub async fn all_imports(&self) -> Result<Vec<(String, String, String, u32)>, libsql::Error> {
        let mut rows = self
            .conn
            .query("SELECT file, module, name, line FROM imports", ())
            .await?;
        let mut imports = Vec::new();
        while let Some(row) = rows.next().await? {
            // module can be NULL in some cases
            let module: Option<String> = row.get(1).ok();
            imports.push((
                row.get(0)?,
                module.unwrap_or_default(),
                row.get(2)?,
                u32::try_from(row.get::<i64>(3)?).unwrap_or(0),
            ));
        }
        Ok(imports)
    }

    /// Load all resolved import edges from the imports table.
    /// Returns Vec<(importer_file, imported_file)> for rows where `resolved_file IS NOT NULL`.
    /// The paths are root-relative strings as stored in the database.
    /// Used by the daemon to build the reverse-dep graph on startup.
    pub async fn all_resolved_import_edges(&self) -> Result<Vec<(String, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, resolved_file FROM imports WHERE resolved_file IS NOT NULL",
                (),
            )
            .await?;
        let mut edges = Vec::new();
        while let Some(row) = rows.next().await? {
            edges.push((row.get(0)?, row.get(1)?));
        }
        Ok(edges)
    }

    /// Load resolved import edges for a specific importer file (root-relative path).
    /// Returns Vec<imported_file> where `resolved_file IS NOT NULL`.
    /// Used by the daemon to update outgoing edges for a changed file.
    pub async fn resolved_imports_for_file(
        &self,
        file: &str,
    ) -> Result<Vec<String>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT resolved_file FROM imports WHERE file = ?1 AND resolved_file IS NOT NULL",
                params![file.to_string()],
            )
            .await?;
        let mut targets = Vec::new();
        while let Some(row) = rows.next().await? {
            targets.push(row.get(0)?);
        }
        Ok(targets)
    }

    /// Load all symbol implements from the symbol_implements table.
    /// Returns Vec<(file, name, interface)>.
    pub async fn all_symbol_implements(
        &self,
    ) -> Result<Vec<(String, String, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query("SELECT file, name, interface FROM symbol_implements", ())
            .await?;
        let mut implements = Vec::new();
        while let Some(row) = rows.next().await? {
            implements.push((row.get(0)?, row.get(1)?, row.get(2)?));
        }
        Ok(implements)
    }

    /// Load all type methods from the type_methods table.
    /// Returns Vec<(file, type_name, method_name)>.
    pub async fn all_type_methods(&self) -> Result<Vec<(String, String, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query("SELECT file, type_name, method_name FROM type_methods", ())
            .await?;
        let mut methods = Vec::new();
        while let Some(row) = rows.next().await? {
            methods.push((row.get(0)?, row.get(1)?, row.get(2)?));
        }
        Ok(methods)
    }

    /// Load all calls with line numbers.
    /// Returns Vec<(caller_file, caller_symbol, callee_name, line)>.
    /// Used by rules for building relations.
    pub async fn all_calls_with_lines(
        &self,
    ) -> Result<Vec<(String, String, String, u32)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT caller_file, caller_symbol, callee_name, line FROM calls",
                (),
            )
            .await?;
        let mut calls = Vec::new();
        while let Some(row) = rows.next().await? {
            calls.push((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                u32::try_from(row.get::<i64>(3)?).unwrap_or(0),
            ));
        }
        Ok(calls)
    }

    /// Load all symbols from the symbols table with full details.
    /// Returns Vec<(file, name, kind, start_line, end_line, parent, visibility, is_impl)>.
    /// Used by test-gaps analysis to classify test context.
    pub async fn all_symbols_with_details(
        &self,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            usize,
            usize,
            Option<String>,
            String,
            bool,
        )>,
        libsql::Error,
    > {
        let mut rows = self
            .conn
            .query(
                "SELECT file, name, kind, start_line, end_line, parent, visibility, is_impl FROM symbols",
                (),
            )
            .await?;
        let mut symbols = Vec::new();
        while let Some(row) = rows.next().await? {
            symbols.push((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
                u64::try_from(row.get::<i64>(4)?).unwrap_or(0) as usize,
                row.get(5).ok(),
                row.get::<String>(6)
                    .unwrap_or_else(|_| "public".to_string()),
                row.get::<i64>(7).unwrap_or(0) != 0,
            ));
        }
        Ok(symbols)
    }

    /// Load all symbol attributes from the symbol_attributes table.
    /// Returns Vec<(file, name, attribute)>.
    pub async fn all_symbol_attributes(
        &self,
    ) -> Result<Vec<(String, String, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query("SELECT file, name, attribute FROM symbol_attributes", ())
            .await?;
        let mut attrs = Vec::new();
        while let Some(row) = rows.next().await? {
            attrs.push((row.get(0)?, row.get(1)?, row.get(2)?));
        }
        Ok(attrs)
    }

    /// Load all calls with qualifiers.
    /// Returns Vec<(caller_file, caller_symbol, callee_name, callee_qualifier, line)>.
    pub async fn all_calls_with_qualifiers(
        &self,
    ) -> Result<Vec<(String, String, String, Option<String>, u32)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT caller_file, caller_symbol, callee_name, callee_qualifier, line FROM calls",
                (),
            )
            .await?;
        let mut calls = Vec::new();
        while let Some(row) = rows.next().await? {
            calls.push((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3).ok(),
                u32::try_from(row.get::<i64>(4)?).unwrap_or(0),
            ));
        }
        Ok(calls)
    }

    /// Convert a module name to possible file paths using the language's trait method.
    /// Returns only paths that exist in the index.
    pub async fn module_to_files(&self, module: &str, source_file: &str) -> Vec<String> {
        // Get language from the source file extension
        let lang = match support_for_path(Path::new(source_file)) {
            Some(l) => l,
            None => return vec![],
        };

        // Get local deps implementation for this language
        let deps = match normalize_local_deps::registry::deps_for_language(lang.name()) {
            Some(d) => d,
            None => return vec![],
        };

        // First try resolve_local_import which handles crate::, super::, self:: properly
        let source_path = self.root.join(source_file);
        if let Some(resolved) = deps.resolve_local_import(module, &source_path, &self.root) {
            // Convert absolute path back to relative path for index lookup
            if let Ok(rel_path) = resolved.strip_prefix(&self.root) {
                let rel_str = rel_path.to_string_lossy().to_string();
                // Verify it exists in index
                if let Ok(mut rows) = self
                    .conn
                    .query(
                        "SELECT 1 FROM files WHERE path = ?1",
                        params![rel_str.clone()],
                    )
                    .await
                    && rows.next().await.ok().flatten().is_some()
                {
                    return vec![rel_str];
                }
            }
        }

        // Fall back to module_name_to_paths for simpler lookups
        let candidates = deps.module_name_to_paths(module);

        // Filter to files that exist in index
        let mut result = Vec::new();
        for path in candidates {
            let mut rows = match self
                .conn
                .query("SELECT 1 FROM files WHERE path = ?1", params![path.clone()])
                .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };
            if rows.next().await.ok().flatten().is_some() {
                result.push(path);
            }
        }
        result
    }

    /// Resolve all unresolved import rows by populating `resolved_file`.
    ///
    /// For each import row where `module IS NOT NULL` and `resolved_file IS NULL`,
    /// calls `module_to_files()` to convert the module specifier to a project-relative
    /// file path and writes it back. Rows that cannot be resolved (external packages,
    /// stdlib, unknown modules) keep `resolved_file = NULL`.
    ///
    /// Safe to call multiple times — only processes rows with `resolved_file IS NULL`.
    pub async fn resolve_all_imports(&self) -> Result<usize, libsql::Error> {
        // Collect distinct (file, module) pairs that still need resolution.
        // We can't mutate while iterating, so collect first.
        let mut rows = self
            .conn
            .query(
                "SELECT DISTINCT file, module FROM imports WHERE module IS NOT NULL AND resolved_file IS NULL",
                (),
            )
            .await?;
        let mut pending: Vec<(String, String)> = Vec::new();
        while let Some(row) = rows.next().await? {
            pending.push((row.get(0)?, row.get(1)?));
        }

        let mut resolved_count = 0;
        for (file, module) in pending {
            let files = self.module_to_files(&module, &file).await;
            if let Some(resolved_file) = files.first() {
                self.conn
                    .execute(
                        "UPDATE imports SET resolved_file = ?1 WHERE file = ?2 AND module = ?3 AND resolved_file IS NULL",
                        params![resolved_file.clone(), file.clone(), module.clone()],
                    )
                    .await?;
                resolved_count += 1;
            }
        }
        Ok(resolved_count)
    }

    /// Resolve call targets: for each call, try to determine which file defines the callee.
    ///
    /// Uses the import graph: if caller_file imports a name that matches callee_name (or its alias),
    /// and that import has a resolved_file, set callee_resolved_file on the call row.
    /// Same-file calls (caller_file has a symbol matching callee_name) also get resolved.
    pub async fn resolve_all_calls(&self) -> Result<usize, libsql::Error> {
        let mut resolved = 0usize;

        // 1. Same-file calls: callee defined in the same file as the caller
        resolved += self
            .conn
            .execute(
                "UPDATE calls SET callee_resolved_file = caller_file
                 WHERE callee_resolved_file IS NULL
                   AND callee_qualifier IS NULL
                   AND EXISTS (
                       SELECT 1 FROM symbols
                       WHERE symbols.file = calls.caller_file
                         AND symbols.name = calls.callee_name
                   )",
                (),
            )
            .await? as usize;

        // 2. Import-resolved calls: callee_name matches an import name (or alias)
        //    that has a resolved_file
        resolved += self
            .conn
            .execute(
                "UPDATE calls SET callee_resolved_file = (
                     SELECT i.resolved_file FROM imports i
                     WHERE i.file = calls.caller_file
                       AND calls.callee_name = COALESCE(i.alias, i.name)
                       AND i.resolved_file IS NOT NULL
                     LIMIT 1
                 )
                 WHERE callee_resolved_file IS NULL
                   AND callee_qualifier IS NULL
                   AND EXISTS (
                       SELECT 1 FROM imports i
                       WHERE i.file = calls.caller_file
                         AND calls.callee_name = COALESCE(i.alias, i.name)
                         AND i.resolved_file IS NOT NULL
                   )",
                (),
            )
            .await? as usize;

        // 3. Qualifier-resolved calls: callee_qualifier matches an import name (or alias)
        //    e.g., `module.foo()` where `module` is imported
        resolved += self
            .conn
            .execute(
                "UPDATE calls SET callee_resolved_file = (
                     SELECT i.resolved_file FROM imports i
                     WHERE i.file = calls.caller_file
                       AND calls.callee_qualifier = COALESCE(i.alias, i.name)
                       AND i.resolved_file IS NOT NULL
                     LIMIT 1
                 )
                 WHERE callee_resolved_file IS NULL
                   AND callee_qualifier IS NOT NULL
                   AND callee_qualifier != 'self'
                   AND EXISTS (
                       SELECT 1 FROM imports i
                       WHERE i.file = calls.caller_file
                         AND calls.callee_qualifier = COALESCE(i.alias, i.name)
                         AND i.resolved_file IS NOT NULL
                   )",
                (),
            )
            .await? as usize;

        // 4. Self-calls: `self.method()` — resolve to the file containing the parent type
        //    The caller's parent type is in the same file, so resolve to caller_file.
        resolved += self
            .conn
            .execute(
                "UPDATE calls SET callee_resolved_file = caller_file
                 WHERE callee_resolved_file IS NULL
                   AND callee_qualifier = 'self'",
                (),
            )
            .await? as usize;

        Ok(resolved)
    }

    /// Check if a file exports (defines) a given symbol
    async fn file_exports_symbol(&self, file: &str, symbol: &str) -> Result<bool, libsql::Error> {
        // Check if symbol is defined in this file (top-level only, parent IS NULL)
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM symbols WHERE file = ?1 AND name = ?2 AND parent IS NULL",
                params![file, symbol],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let count: i64 = row.get(0)?;
            Ok(count > 0)
        } else {
            Ok(false)
        }
    }

    /// Resolve a name in a file's context to its source module
    /// Returns: (source_module, original_name) if found
    pub async fn resolve_import(
        &self,
        file: &str,
        name: &str,
    ) -> Result<Option<(String, String)>, libsql::Error> {
        // Check for direct import or alias
        let mut rows = self
            .conn
            .query(
                "SELECT module, name FROM imports WHERE file = ?1 AND (name = ?2 OR alias = ?2)",
                params![file, name],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            let module: Option<String> = row.get(0)?;
            let orig_name: String = row.get(1)?;
            if let Some(module) = module {
                return Ok(Some((module, orig_name)));
            } else {
                // Plain import (import X), module is the name
                return Ok(Some((orig_name.clone(), orig_name)));
            }
        }

        // Check for wildcard imports - name could come from any of them
        let mut rows = self
            .conn
            .query(
                "SELECT module FROM imports WHERE file = ?1 AND name = '*'",
                params![file],
            )
            .await?;
        let mut wildcards = Vec::new();
        while let Some(row) = rows.next().await? {
            if let Ok(Some(module)) = row.get::<Option<String>>(0) {
                wildcards.push(module);
            }
        }

        // Check each wildcard source to see if it exports the symbol
        for module in &wildcards {
            let files = self.module_to_files(module, file).await;
            for module_file in files {
                if self.file_exports_symbol(&module_file, name).await? {
                    return Ok(Some((module.clone(), name.to_string())));
                }
            }
        }

        // Fallback: if we have wildcards but couldn't verify, return first as possibility
        // This handles external modules (stdlib, third-party) we can't resolve
        if !wildcards.is_empty() {
            return Ok(Some((wildcards[0].clone(), name.to_string())));
        }

        Ok(None)
    }

    /// Find which files import a given module
    pub async fn find_importers(
        &self,
        module: &str,
    ) -> Result<Vec<(String, String, usize)>, libsql::Error> {
        let pattern = format!("{}%", module);
        let mut rows = self
            .conn
            .query(
                "SELECT file, name, line FROM imports WHERE module = ?1 OR module LIKE ?2",
                params![module, pattern],
            )
            .await?;
        let mut importers = Vec::new();
        while let Some(row) = rows.next().await? {
            importers.push((
                row.get(0)?,
                row.get(1)?,
                u64::try_from(row.get::<i64>(2)?).unwrap_or(0) as usize,
            ));
        }
        Ok(importers)
    }

    /// Check whether a file already has an import named `name` (as `name` or `alias`).
    /// Used for rename conflict detection.
    pub async fn has_import_named(&self, file: &str, name: &str) -> Result<bool, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM imports WHERE file = ?1 AND (name = ?2 OR alias = ?2)",
                params![file, name],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let count: i64 = row.get(0)?;
            Ok(count > 0)
        } else {
            Ok(false)
        }
    }

    /// Find files that import a specific symbol by name.
    /// Returns: (file, imported_name, alias, line)
    /// Useful for rename: find all files that need their import statement updated.
    pub async fn find_symbol_importers(
        &self,
        symbol_name: &str,
    ) -> Result<Vec<(String, String, Option<String>, usize)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, name, alias, line FROM imports WHERE name = ?1",
                params![symbol_name],
            )
            .await?;
        let mut importers = Vec::new();
        while let Some(row) = rows.next().await? {
            importers.push((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
            ));
        }
        Ok(importers)
    }

    /// Get method names for a type (interface/class) in a specific file.
    /// Used for cross-file interface implementation detection.
    pub async fn get_type_methods(
        &self,
        file: &str,
        type_name: &str,
    ) -> Result<Vec<String>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT method_name FROM type_methods WHERE file = ?1 AND type_name = ?2",
                params![file, type_name],
            )
            .await?;
        let mut methods = Vec::new();
        while let Some(row) = rows.next().await? {
            methods.push(row.get(0)?);
        }
        Ok(methods)
    }

    /// Find files that define a type by name.
    /// Returns all files that have a type (interface/class) with the given name.
    pub async fn find_type_definitions(
        &self,
        type_name: &str,
    ) -> Result<Vec<String>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT DISTINCT file FROM type_methods WHERE type_name = ?1",
                params![type_name],
            )
            .await?;
        let mut files = Vec::new();
        while let Some(row) = rows.next().await? {
            files.push(row.get(0)?);
        }
        Ok(files)
    }

    /// Refresh the call graph by parsing all supported source files
    /// This is more expensive than file refresh since it parses every file
    /// Uses parallel processing for parsing, sequential insertion for SQLite
    pub async fn refresh_call_graph(&mut self) -> Result<CallGraphStats, libsql::Error> {
        // Get all indexed source files
        let files: Vec<String> = {
            let sql = format!(
                "SELECT path FROM files WHERE is_dir = 0 AND ({})",
                source_extensions_sql_filter()
            );
            let mut rows = self.conn.query(&sql, ()).await?;
            let mut files = Vec::new();
            while let Some(row) = rows.next().await? {
                let path: String = row.get(0)?;
                files.push(path);
            }
            files
        };

        // Parse all files in parallel
        // Each thread gets its own SymbolParser (tree-sitter parsers have mutable state)
        let root = self.root.clone();
        let pb = if self.progress && std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            let pb = ProgressBar::new(files.len() as u64);
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.cyan} Parsing symbols... [{bar:30.cyan/dim}] {pos}/{len} files [{elapsed_precise}]",
                )
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("##-"),
            );
            pb
        } else {
            ProgressBar::hidden()
        };
        let parsed_data: Vec<ParsedFileData> = files
            .par_iter()
            .progress_with(pb.clone())
            .filter_map(|file_path| {
                let full_path = root.join(file_path);
                let content = std::fs::read_to_string(&full_path).ok()?;

                // Each thread creates its own parser
                let mut parser = SymbolParser::new();
                let symbols = parser.parse_file(&full_path, &content);

                let mut sym_data = Vec::with_capacity(symbols.len());
                let mut call_data = Vec::new();

                for sym in &symbols {
                    sym_data.push(ParsedSymbol {
                        name: sym.name.clone(),
                        kind: sym.kind.as_str().to_string(),
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                        parent: sym.parent.clone(),
                        visibility: sym.visibility.as_str().to_string(),
                        attributes: sym.attributes.clone(),
                        is_interface_impl: sym.is_interface_impl,
                        implements: sym.implements.clone(),
                        docstring: sym.docstring.clone(),
                    });

                    // Only index calls for functions/methods
                    let kind = sym.kind.as_str();
                    if kind == "function" || kind == "method" {
                        let calls = parser.find_callees_for_symbol(&full_path, &content, sym);
                        for (callee_name, line, qualifier, access) in calls {
                            call_data.push((
                                sym.name.clone(),
                                callee_name,
                                qualifier,
                                access,
                                line,
                            ));
                        }
                    }
                }

                // Parse imports using trait-based extraction (works for all supported languages)
                let imports = parser.parse_imports(&full_path, &content);

                // Extract type methods for cross-file interface resolution
                // We need to use the full symbol extraction to get hierarchy
                let extractor = crate::extract::Extractor::new();
                let extract_result = extractor.extract(&full_path, &content);
                let mut type_methods = Vec::new();
                for sym in &extract_result.symbols {
                    if matches!(
                        sym.kind,
                        normalize_languages::SymbolKind::Interface
                            | normalize_languages::SymbolKind::Class
                            | normalize_languages::SymbolKind::Trait
                            | normalize_languages::SymbolKind::Struct
                    ) {
                        for child in &sym.children {
                            if matches!(
                                child.kind,
                                normalize_languages::SymbolKind::Method
                                    | normalize_languages::SymbolKind::Function
                            ) {
                                type_methods.push((sym.name.clone(), child.name.clone()));
                            }
                        }
                    }
                }

                // Extract type references using tree-sitter queries
                let type_refs = parser.find_type_refs(&full_path, &content);

                Some(ParsedFileData {
                    file_path: file_path.clone(),
                    symbols: sym_data,
                    calls: call_data,
                    imports,
                    type_methods,
                    type_refs,
                })
            })
            .collect();

        pb.finish_and_clear();

        let pb_insert = if self.progress && std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            let pb = ProgressBar::new(parsed_data.len() as u64);
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.cyan} Storing index... [{bar:30.cyan/dim}] {pos}/{len} files [{elapsed_precise}]",
                )
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("##-"),
            );
            pb
        } else {
            ProgressBar::hidden()
        };

        self.conn.execute("BEGIN", ()).await?;

        // Clear existing data
        self.conn.execute("DELETE FROM symbols", ()).await?;
        self.conn.execute("DELETE FROM calls", ()).await?;
        self.conn.execute("DELETE FROM imports", ()).await?;
        self.conn.execute("DELETE FROM type_methods", ()).await?;
        self.conn.execute("DELETE FROM type_refs", ()).await?;
        self.conn
            .execute("DELETE FROM symbol_attributes", ())
            .await?;
        self.conn
            .execute("DELETE FROM symbol_implements", ())
            .await?;

        let mut symbol_count = 0;
        let mut call_count = 0;
        let mut import_count = 0;

        for data in &parsed_data {
            for sym in &data.symbols {
                self.conn.execute(
                    "INSERT INTO symbols (file, name, kind, start_line, end_line, parent, visibility, is_impl) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![data.file_path.clone(), sym.name.clone(), sym.kind.clone(), sym.start_line as i64, sym.end_line as i64, sym.parent.clone(), sym.visibility.clone(), sym.is_interface_impl as i64],
                ).await?;
                for attr in &sym.attributes {
                    self.conn.execute(
                        "INSERT INTO symbol_attributes (file, name, attribute) VALUES (?1, ?2, ?3)",
                        params![data.file_path.clone(), sym.name.clone(), attr.clone()],
                    ).await?;
                }
                if let Some(doc) = &sym.docstring {
                    self.conn.execute(
                        "INSERT INTO symbol_attributes (file, name, attribute) VALUES (?1, ?2, ?3)",
                        params![data.file_path.clone(), sym.name.clone(), format!("doc:{doc}")],
                    ).await?;
                }
                for iface in &sym.implements {
                    self.conn.execute(
                        "INSERT INTO symbol_implements (file, name, interface) VALUES (?1, ?2, ?3)",
                        params![data.file_path.clone(), sym.name.clone(), iface.clone()],
                    ).await?;
                }
                symbol_count += 1;
            }

            for (caller_symbol, callee_name, qualifier, access, line) in &data.calls {
                self.conn.execute(
                    "INSERT INTO calls (caller_file, caller_symbol, callee_name, callee_qualifier, access, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![data.file_path.clone(), caller_symbol.clone(), callee_name.clone(), qualifier.clone(), access.clone(), *line as i64],
                ).await?;
                call_count += 1;
            }

            for imp in &data.imports {
                self.conn.execute(
                    "INSERT INTO imports (file, module, name, alias, line) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![data.file_path.clone(), imp.module.clone(), imp.name.clone(), imp.alias.clone(), imp.line as i64],
                ).await?;
                import_count += 1;
            }

            for (type_name, method_name) in &data.type_methods {
                self.conn.execute(
                    "INSERT OR IGNORE INTO type_methods (file, type_name, method_name) VALUES (?1, ?2, ?3)",
                    params![data.file_path.clone(), type_name.clone(), method_name.clone()],
                ).await?;
            }

            for tr in &data.type_refs {
                self.conn.execute(
                    "INSERT INTO type_refs (file, source_symbol, target_type, kind, line) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![data.file_path.clone(), tr.source_symbol.clone(), tr.target_type.clone(), tr.kind.as_str(), tr.line as i64],
                ).await?;
            }
            pb_insert.inc(1);
        }

        pb_insert.finish_and_clear();

        self.conn.execute("COMMIT", ()).await?;

        // Resolve import module specifiers to root-relative file paths now that all
        // files are indexed. Must run after COMMIT so module_to_files() can query them.
        self.resolve_all_imports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_imports error: {}", e);
            0
        });
        // Resolve call targets using the now-populated import graph.
        self.resolve_all_calls().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_calls error: {}", e);
            0
        });

        Ok(CallGraphStats {
            symbols: symbol_count,
            calls: call_count,
            imports: import_count,
        })
    }

    /// Reindex specific files: delete old data and re-extract symbols/calls/imports.
    /// Expects to be called inside a transaction.
    async fn reindex_files(
        &self,
        deleted_files: &[String],
        changed_files: &[String],
    ) -> Result<CallGraphStats, libsql::Error> {
        // Remove data for deleted/modified files
        for path in deleted_files.iter().chain(changed_files.iter()) {
            self.conn
                .execute("DELETE FROM symbols WHERE file = ?1", params![path.clone()])
                .await?;
            self.conn
                .execute(
                    "DELETE FROM calls WHERE caller_file = ?1",
                    params![path.clone()],
                )
                .await?;
            self.conn
                .execute("DELETE FROM imports WHERE file = ?1", params![path.clone()])
                .await?;
            self.conn
                .execute(
                    "DELETE FROM symbol_attributes WHERE file = ?1",
                    params![path.clone()],
                )
                .await?;
            self.conn
                .execute(
                    "DELETE FROM symbol_implements WHERE file = ?1",
                    params![path.clone()],
                )
                .await?;
            self.conn
                .execute(
                    "DELETE FROM type_refs WHERE file = ?1",
                    params![path.clone()],
                )
                .await?;
        }

        let mut parser = SymbolParser::new();
        let mut symbol_count = 0;
        let mut call_count = 0;
        let mut import_count = 0;

        // Parse changed files
        for file_path in changed_files {
            let full_path = self.root.join(file_path);
            let content = match std::fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let symbols = parser.parse_file(&full_path, &content);

            for sym in &symbols {
                self.conn.execute(
                    "INSERT INTO symbols (file, name, kind, start_line, end_line, parent, visibility, is_impl) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![file_path.clone(), sym.name.clone(), sym.kind.as_str(), sym.start_line as i64, sym.end_line as i64, sym.parent.clone(), sym.visibility.as_str(), sym.is_interface_impl as i64],
                ).await?;
                for attr in &sym.attributes {
                    self.conn.execute(
                        "INSERT INTO symbol_attributes (file, name, attribute) VALUES (?1, ?2, ?3)",
                        params![file_path.clone(), sym.name.clone(), attr.clone()],
                    ).await?;
                }
                if let Some(doc) = &sym.docstring {
                    self.conn.execute(
                        "INSERT INTO symbol_attributes (file, name, attribute) VALUES (?1, ?2, ?3)",
                        params![file_path.clone(), sym.name.clone(), format!("doc:{doc}")],
                    ).await?;
                }
                for iface in &sym.implements {
                    self.conn.execute(
                        "INSERT INTO symbol_implements (file, name, interface) VALUES (?1, ?2, ?3)",
                        params![file_path.clone(), sym.name.clone(), iface.clone()],
                    ).await?;
                }
                symbol_count += 1;

                let kind = sym.kind.as_str();
                if kind == "function" || kind == "method" {
                    let calls = parser.find_callees_for_symbol(&full_path, &content, sym);
                    for (callee_name, line, qualifier, access) in calls {
                        self.conn.execute(
                            "INSERT INTO calls (caller_file, caller_symbol, callee_name, callee_qualifier, access, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                            params![file_path.clone(), sym.name.clone(), callee_name, qualifier, access, line as i64],
                        ).await?;
                        call_count += 1;
                    }
                }
            }

            // Parse imports using trait-based extraction (works for all supported languages)
            let imports = parser.parse_imports(&full_path, &content);
            for imp in imports {
                self.conn.execute(
                    "INSERT INTO imports (file, module, name, alias, line) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![file_path.clone(), imp.module, imp.name, imp.alias, imp.line as i64],
                ).await?;
                import_count += 1;
            }

            // Extract type references
            let type_refs = parser.find_type_refs(&full_path, &content);
            for tr in type_refs {
                self.conn.execute(
                    "INSERT INTO type_refs (file, source_symbol, target_type, kind, line) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![file_path.clone(), tr.source_symbol, tr.target_type, tr.kind.as_str(), tr.line as i64],
                ).await?;
            }
        }

        Ok(CallGraphStats {
            symbols: symbol_count,
            calls: call_count,
            imports: import_count,
        })
    }

    /// Incrementally update call graph for changed files only.
    /// Much faster than full refresh when few files changed.
    pub async fn incremental_call_graph_refresh(
        &mut self,
    ) -> Result<CallGraphStats, libsql::Error> {
        let changed = self.get_changed_files().await?;

        // Only process supported source and data files
        let changed_files: Vec<String> = changed
            .added
            .into_iter()
            .chain(changed.modified.into_iter())
            .filter(|f| is_source_file(f))
            .collect();

        let deleted_source_files: Vec<String> = changed
            .deleted
            .into_iter()
            .filter(|f| is_source_file(f))
            .collect();

        if changed_files.is_empty() && deleted_source_files.is_empty() {
            return Ok(CallGraphStats::default());
        }

        self.conn.execute("BEGIN", ()).await?;
        let stats = self
            .reindex_files(&deleted_source_files, &changed_files)
            .await?;
        self.conn.execute("COMMIT", ()).await?;

        // Resolve any newly inserted imports to root-relative file paths.
        self.resolve_all_imports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_imports error: {}", e);
            0
        });
        // Resolve call targets using the now-populated import graph.
        self.resolve_all_calls().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_calls error: {}", e);
            0
        });

        Ok(stats)
    }

    /// Update the index for a single file (used by LSP on save).
    /// Skips filesystem walk — directly reindexes the given path and resolves imports/calls.
    pub async fn update_file(&mut self, rel_path: &str) -> Result<CallGraphStats, libsql::Error> {
        let full_path = self.root.join(rel_path);
        let exists = full_path.exists();

        // Update the files table mtime
        if exists {
            let metadata = std::fs::metadata(&full_path).ok();
            let mtime = metadata
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            self.conn
                .execute(
                    "UPDATE files SET mtime = ?1 WHERE path = ?2",
                    params![mtime, rel_path.to_string()],
                )
                .await?;
        }

        if !is_source_file(rel_path) {
            return Ok(CallGraphStats::default());
        }

        self.conn.execute("BEGIN", ()).await?;
        let stats = if exists {
            self.reindex_files(&[], &[rel_path.to_string()]).await?
        } else {
            self.reindex_files(&[rel_path.to_string()], &[]).await?
        };
        self.conn.execute("COMMIT", ()).await?;

        self.resolve_all_imports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_imports error: {}", e);
            0
        });
        self.resolve_all_calls().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_calls error: {}", e);
            0
        });

        Ok(stats)
    }

    /// Check if call graph needs refresh
    #[allow(dead_code)] // FileIndex API - used by daemon
    pub async fn needs_call_graph_refresh(&self) -> bool {
        self.call_graph_stats().await.unwrap_or_default().symbols == 0
    }

    /// Find files matching a query using LIKE (fast pre-filter)
    /// Splits query by whitespace/separators and requires all parts to match
    /// Special case: queries starting with '.' are treated as extension patterns
    pub async fn find_like(&self, query: &str) -> Result<Vec<IndexedFile>, libsql::Error> {
        // Handle extension patterns (e.g., ".rs", ".py")
        if query.starts_with('.') && !query.contains('/') {
            let pattern = format!("%{}", query.to_lowercase());
            let mut rows = self.conn.query(
                "SELECT path, is_dir, mtime, lines FROM files WHERE LOWER(path) LIKE ?1 LIMIT 1000",
                params![pattern],
            ).await?;
            let mut files = Vec::new();
            while let Some(row) = rows.next().await? {
                files.push(IndexedFile {
                    path: row.get(0)?,
                    is_dir: row.get::<i64>(1)? != 0,
                    mtime: row.get(2)?,
                    lines: u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
                });
            }
            return Ok(files);
        }

        // Normalize query: split on whitespace and common separators (but not '.')
        let parts: Vec<&str> = query
            .split(|c: char| c.is_whitespace() || c == '_' || c == '-')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            return Ok(Vec::new());
        }

        // Cap to 4 parts before building SQL so ?1..?N matches the bound params count.
        let parts: Vec<&str> = parts.into_iter().take(4).collect();

        // Build WHERE clause: LOWER(path) LIKE '%part1%' AND LOWER(path) LIKE '%part2%' ...
        let conditions: Vec<String> = (0..parts.len())
            .map(|i| format!("LOWER(path) LIKE ?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT path, is_dir, mtime, lines FROM files WHERE {} LIMIT 50",
            conditions.join(" AND ")
        );

        let patterns: Vec<String> = parts
            .iter()
            .map(|p| format!("%{}%", p.to_lowercase()))
            .collect();

        // For dynamic params, we need to build them differently
        // libsql doesn't support dynamic parameter slices the same way
        // Use a simpler approach for up to common cases
        let mut files = Vec::new();
        let mut rows = match patterns.len() {
            1 => self.conn.query(&sql, params![patterns[0].clone()]).await?,
            2 => {
                self.conn
                    .query(&sql, params![patterns[0].clone(), patterns[1].clone()])
                    .await?
            }
            3 => {
                self.conn
                    .query(
                        &sql,
                        params![
                            patterns[0].clone(),
                            patterns[1].clone(),
                            patterns[2].clone()
                        ],
                    )
                    .await?
            }
            4 => {
                self.conn
                    .query(
                        &sql,
                        params![
                            patterns[0].clone(),
                            patterns[1].clone(),
                            patterns[2].clone(),
                            patterns[3].clone()
                        ],
                    )
                    .await?
            }
            // parts is capped to 4 above, so len > 4 is unreachable
            _ => unreachable!("parts capped to 4"),
        };

        while let Some(row) = rows.next().await? {
            files.push(IndexedFile {
                path: row.get(0)?,
                is_dir: row.get::<i64>(1)? != 0,
                mtime: row.get(2)?,
                lines: u64::try_from(row.get::<i64>(3)?).unwrap_or(0) as usize,
            });
        }
        Ok(files)
    }

    /// Rebuild (or incrementally update) the co-change edges table from git history.
    ///
    /// When `since_commit` is `None`, performs a full rebuild: clears the table and walks
    /// all commits. When `since_commit` is `Some(sha)`, walks only commits after that SHA
    /// and merges counts into the existing table before re-applying the per-file fanout cap.
    ///
    /// Algorithm:
    /// 1. Walk commits via gix (pure-Rust, no `git` binary required).
    /// 2. For each commit: skip if it touches >50 files (large mechanical commit, no signal).
    /// 3. For each pair of source files in a commit: increment co-change count.
    /// 4. Apply filters: drop pairs with count < 2, cap each file to top 20 partners.
    /// 5. Upsert into `co_change_edges`.
    /// 6. Record HEAD SHA in `meta.co_change_last_commit` for incremental use.
    pub async fn rebuild_co_change_edges(
        &self,
        since_commit: Option<&str>,
    ) -> Result<usize, libsql::Error> {
        use std::collections::HashMap;

        let root = &self.root;

        // Open gix repository. If not a git repo, silently skip (not an error).
        let repo = match open_gix_repo(root) {
            Some(r) => r,
            None => {
                tracing::debug!("co-change: no git repository found at {:?}, skipping", root);
                return Ok(0);
            }
        };

        let head_sha = match repo.head_id() {
            Ok(id) => id.to_string(),
            Err(_) => return Ok(0),
        };

        // Walk commits, collecting per-commit file lists.
        let commit_files = walk_commits_for_co_change(&repo, since_commit);

        if commit_files.is_empty() && since_commit.is_none() {
            // No history (or empty repo): ensure table is cleared and metadata stored.
            self.conn.execute("DELETE FROM co_change_edges", ()).await?;
            self.conn
                .execute(
                    "INSERT OR REPLACE INTO meta (key, value) VALUES ('co_change_last_commit', ?1)",
                    params![head_sha],
                )
                .await?;
            return Ok(0);
        }

        // For incremental: load existing counts from DB, merge new counts, re-apply cap.
        // For full: start fresh.
        let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();

        if since_commit.is_some() {
            // Load existing edges into the map so we can merge.
            let mut rows = self
                .conn
                .query("SELECT file_a, file_b, count FROM co_change_edges", ())
                .await?;
            while let Some(row) = rows.next().await? {
                let a: String = row.get(0)?;
                let b: String = row.get(1)?;
                let c: i64 = row.get(2)?;
                pair_counts.insert((a, b), c as usize);
            }
        }

        // Accumulate new commit data.
        for files in &commit_files {
            // Files are already filtered to source files only.
            if files.len() > 50 || files.len() < 2 {
                continue;
            }
            let mut sorted = files.clone();
            sorted.sort_unstable();
            sorted.dedup();
            for i in 0..sorted.len() {
                for j in (i + 1)..sorted.len() {
                    let key = (sorted[i].clone(), sorted[j].clone());
                    *pair_counts.entry(key).or_default() += 1;
                }
            }
        }

        // Apply filters: drop count < 2, apply per-file top-20 fanout cap.
        pair_counts.retain(|_, v| *v >= 2);
        let pair_counts = apply_fanout_cap(pair_counts, 20);

        // Write to DB.
        if since_commit.is_some() {
            // Full replace: clear and reinsert (we have the full merged set).
            self.conn.execute("DELETE FROM co_change_edges", ()).await?;
        } else {
            self.conn.execute("DELETE FROM co_change_edges", ()).await?;
        }

        let mut inserted = 0usize;
        for ((a, b), count) in &pair_counts {
            self.conn.execute(
                "INSERT OR REPLACE INTO co_change_edges (file_a, file_b, count) VALUES (?1, ?2, ?3)",
                params![a.clone(), b.clone(), *count as i64],
            ).await?;
            inserted += 1;
        }

        // Record the HEAD SHA so the next incremental run knows where to resume.
        self.conn
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('co_change_last_commit', ?1)",
                params![head_sha],
            )
            .await?;

        Ok(inserted)
    }

    /// Query co-change edges from the index.
    ///
    /// Returns pairs `(file_a, file_b, count)` where count >= `min_count`.
    /// Returns `Ok(None)` if the `co_change_edges` table is empty (not yet built),
    /// so callers can fall back to the git walk.
    pub async fn query_co_change_edges(
        &self,
        min_count: usize,
    ) -> Result<Option<Vec<(String, String, usize)>>, libsql::Error> {
        // Check if the table has any data.
        let mut check = self
            .conn
            .query("SELECT COUNT(*) FROM co_change_edges", ())
            .await?;
        let total: i64 = if let Some(row) = check.next().await? {
            row.get(0)?
        } else {
            0
        };
        if total == 0 {
            return Ok(None);
        }

        let mut rows = self
            .conn
            .query(
                "SELECT file_a, file_b, count FROM co_change_edges WHERE count >= ?1",
                params![min_count as i64],
            )
            .await?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            let a: String = row.get(0)?;
            let b: String = row.get(1)?;
            let c: i64 = row.get(2)?;
            result.push((a, b, c as usize));
        }
        Ok(Some(result))
    }

    /// Return the stored HEAD SHA from the last co-change rebuild, if any.
    pub async fn co_change_last_commit(&self) -> Option<String> {
        let mut rows = self
            .conn
            .query(
                "SELECT value FROM meta WHERE key = 'co_change_last_commit'",
                (),
            )
            .await
            .ok()?;
        let row = rows.next().await.ok()??;
        row.get(0).ok()
    }

    // -------------------------------------------------------------------------
    // Diagnostics cache (daemon use only)
    // -------------------------------------------------------------------------

    /// Persist serialized diagnostics JSON for one engine ("syntax", "fact", "native").
    /// Replaces any previous value for that engine.
    pub async fn save_diagnostics_json(
        &self,
        engine: &str,
        issues_json: &str,
    ) -> Result<(), libsql::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO daemon_diagnostics (engine, issues_json, updated_at)
                 VALUES (?1, ?2, ?3)",
                params![engine.to_string(), issues_json.to_string(), now],
            )
            .await?;
        Ok(())
    }

    /// Load serialized diagnostics JSON for one engine.
    /// Returns `None` if no data has been saved for that engine yet.
    pub async fn load_diagnostics_json(
        &self,
        engine: &str,
    ) -> Result<Option<String>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT issues_json FROM daemon_diagnostics WHERE engine = ?1",
                params![engine.to_string()],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }
}

// =============================================================================
// Co-change helpers (not on FileIndex — free functions to keep impl clean)
// =============================================================================

/// Open a gix repository at or containing `root`.
fn open_gix_repo(root: &std::path::Path) -> Option<gix::Repository> {
    gix::discover(root)
        .ok()
        .map(|r| r.into_sync().to_thread_local())
}

/// Walk commits via gix, returning per-commit lists of *source* files changed.
///
/// If `since_commit` is `Some(sha)`, only commits after (exclusive) that SHA are returned.
/// Commits are yielded oldest-first from the HEAD ancestry.
fn walk_commits_for_co_change(
    repo: &gix::Repository,
    since_commit: Option<&str>,
) -> Vec<Vec<String>> {
    let head_id = match repo.head_id() {
        Ok(id) => id,
        Err(_) => return Vec::new(),
    };
    let walk = match head_id.ancestors().all() {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };

    // If since_commit is specified, resolve it to an ObjectId for fast comparison.
    let stop_id: Option<gix::hash::ObjectId> = since_commit.and_then(|sha| sha.parse().ok());

    let mut result = Vec::new();

    for info in walk {
        let Ok(info) = info else { continue };
        let commit_id = info.id();

        // Stop when we hit the commit we already processed.
        if let Some(ref stop) = stop_id
            && commit_id == *stop
        {
            break;
        }

        let Ok(commit) = info.object() else { continue };
        let Ok(tree) = commit.tree() else { continue };

        let parent_tree = info
            .parent_ids()
            .next()
            .and_then(|pid| pid.object().ok())
            .and_then(|obj| obj.into_commit().tree().ok());

        let changes = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let files: Vec<String> = changes
            .into_iter()
            .filter_map(|change| {
                use gix::object::tree::diff::ChangeDetached;
                let location = match change {
                    ChangeDetached::Addition { location, .. } => location,
                    ChangeDetached::Deletion { location, .. } => location,
                    ChangeDetached::Modification { location, .. } => location,
                    ChangeDetached::Rewrite {
                        source_location, ..
                    } => source_location,
                };
                let path_str = String::from_utf8_lossy(&location).into_owned();
                // Only include source files (those with a supported language extension).
                if is_source_file(&path_str) {
                    Some(path_str)
                } else {
                    None
                }
            })
            .collect();

        if files.len() >= 2 {
            result.push(files);
        }
    }

    result
}

/// Apply a per-file fanout cap: for each file, keep only its top `cap` partners by count.
///
/// Returns a new HashMap with entries pruned to satisfy the cap.
fn apply_fanout_cap(
    pair_counts: std::collections::HashMap<(String, String), usize>,
    cap: usize,
) -> std::collections::HashMap<(String, String), usize> {
    use std::collections::HashMap;

    // Build per-file partner lists.
    let mut file_partners: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for ((a, b), count) in &pair_counts {
        file_partners
            .entry(a.clone())
            .or_default()
            .push((b.clone(), *count));
        file_partners
            .entry(b.clone())
            .or_default()
            .push((a.clone(), *count));
    }

    // For each file, keep only the top `cap` partners.
    let mut allowed: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for (file, mut partners) in file_partners {
        partners.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        partners.truncate(cap);
        for (partner, _) in partners {
            // Canonical key: lexicographically smaller goes first.
            let key = if file <= partner {
                (file.clone(), partner)
            } else {
                (partner, file.clone())
            };
            allowed.insert(key);
        }
    }

    pair_counts
        .into_iter()
        .filter(|(k, _)| allowed.contains(k))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_index_creation() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/myapp")).unwrap();
        fs::write(dir.path().join("src/myapp/cli.py"), "").unwrap();
        fs::write(dir.path().join("src/myapp/dwim.py"), "").unwrap();

        let mut index = FileIndex::open(&dir.path().join("index.sqlite"), dir.path())
            .await
            .unwrap();
        assert!(index.needs_refresh().await);

        let count = index.refresh().await.unwrap();
        assert!(count >= 2);

        // Should find files by name
        let matches = index.find_by_name("cli.py").await.unwrap();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].path.ends_with("cli.py"));
    }

    #[tokio::test]
    async fn test_find_by_stem() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/test.py"), "").unwrap();
        fs::write(dir.path().join("src/test.rs"), "").unwrap();

        let mut index = FileIndex::open(&dir.path().join("index.sqlite"), dir.path())
            .await
            .unwrap();
        index.refresh().await.unwrap();

        let matches = index.find_by_stem("test").await.unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[tokio::test]
    async fn test_wildcard_import_resolution() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/mylib")).unwrap();
        // Module that exports MyClass
        fs::write(
            dir.path().join("src/mylib/exports.py"),
            "class MyClass: pass",
        )
        .unwrap();
        // Module that exports OtherThing
        fs::write(
            dir.path().join("src/mylib/other.py"),
            "def OtherThing(): pass",
        )
        .unwrap();
        // Consumer with wildcard imports
        fs::write(
            dir.path().join("src/consumer.py"),
            "from mylib.exports import *\nfrom mylib.other import *\nMyClass()",
        )
        .unwrap();

        let mut index = FileIndex::open(&dir.path().join("index.sqlite"), dir.path())
            .await
            .unwrap();
        index.refresh().await.unwrap();
        index.refresh_call_graph().await.unwrap();

        // Now resolve MyClass - should find it in mylib.exports
        let result = index
            .resolve_import("src/consumer.py", "MyClass")
            .await
            .unwrap();
        assert!(result.is_some(), "Should resolve MyClass");
        let (module, name) = result.unwrap();
        assert_eq!(module, "mylib.exports");
        assert_eq!(name, "MyClass");

        // Resolve OtherThing - should find it in mylib.other
        let result = index
            .resolve_import("src/consumer.py", "OtherThing")
            .await
            .unwrap();
        assert!(result.is_some(), "Should resolve OtherThing");
        let (module, name) = result.unwrap();
        assert_eq!(module, "mylib.other");
        assert_eq!(name, "OtherThing");
    }

    #[tokio::test]
    async fn test_method_call_resolution() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        // A class with methods that call each other
        let class_code = r#"
class MyClass:
    def method_a(self):
        self.method_b()

    def method_b(self):
        pass

    def method_c(self):
        self.method_b()
"#;
        fs::write(dir.path().join("src/myclass.py"), class_code).unwrap();

        let mut index = FileIndex::open(&dir.path().join("index.sqlite"), dir.path())
            .await
            .unwrap();
        index.refresh().await.unwrap();
        index.refresh_call_graph().await.unwrap();

        // Find callers of method_b - should include method_a and method_c
        let callers = index
            .find_callers("method_b", "src/myclass.py")
            .await
            .unwrap();
        assert!(!callers.is_empty(), "Should find callers of method_b");

        let caller_names: Vec<&str> = callers
            .iter()
            .map(|(_, name, _, _)| name.as_str())
            .collect();
        assert!(
            caller_names.contains(&"method_a"),
            "method_a should call method_b"
        );
        assert!(
            caller_names.contains(&"method_c"),
            "method_c should call method_b"
        );

        // Find callers of MyClass.method_b - more specific
        let callers = index
            .find_callers("MyClass.method_b", "src/myclass.py")
            .await
            .unwrap();
        assert!(
            !callers.is_empty(),
            "Should find callers of MyClass.method_b"
        );
    }

    /// Regression test: find_callers must not return callers of a same-named function
    /// in a different module. Two modules define `helper()`, and `main.py` imports only
    /// one of them. `find_callers("helper", "src/utils_a.py")` must not include calls
    /// that target `src/utils_b.py`'s `helper()`.
    #[tokio::test]
    async fn test_find_callers_cross_module_disambiguation() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();

        // Two modules with the same function name
        fs::write(
            dir.path().join("src/utils_a.py"),
            "def helper():\n    return 'A'\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("src/utils_b.py"),
            "def helper():\n    return 'B'\n",
        )
        .unwrap();

        // caller_a.py imports from utils_a and calls helper()
        fs::write(
            dir.path().join("src/caller_a.py"),
            "from utils_a import helper\n\ndef do_a():\n    helper()\n",
        )
        .unwrap();

        // caller_b.py imports from utils_b and calls helper()
        fs::write(
            dir.path().join("src/caller_b.py"),
            "from utils_b import helper\n\ndef do_b():\n    helper()\n",
        )
        .unwrap();

        let mut index = FileIndex::open(&dir.path().join("index.sqlite"), dir.path())
            .await
            .unwrap();
        index.refresh().await.unwrap();
        index.refresh_call_graph().await.unwrap();

        // Check whether imports got resolved (depends on normalize-local-deps Python support)
        let mut rows = index
            .connection()
            .query(
                "SELECT file, resolved_file FROM imports WHERE name = 'helper' ORDER BY file",
                (),
            )
            .await
            .unwrap();
        let mut import_resolution: Vec<(String, Option<String>)> = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            import_resolution.push((row.get(0).unwrap(), row.get(1).unwrap()));
        }

        // Check whether calls got resolved
        let mut rows = index
            .connection()
            .query(
                "SELECT caller_file, callee_name, callee_resolved_file FROM calls WHERE callee_name = 'helper' ORDER BY caller_file",
                (),
            )
            .await
            .unwrap();
        let mut call_resolution: Vec<(String, String, Option<String>)> = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            call_resolution.push((
                row.get(0).unwrap(),
                row.get(1).unwrap(),
                row.get(2).unwrap(),
            ));
        }

        // Ask for callers of utils_a's helper
        let callers = index
            .find_callers("helper", "src/utils_a.py")
            .await
            .unwrap();
        let caller_files: Vec<&str> = callers.iter().map(|(f, _, _, _)| f.as_str()).collect();

        // When imports are resolved, disambiguation is precise — only the correct
        // caller appears. When unresolved (no LocalDeps for test setup), both
        // callers may appear via the NULL fallback. Either way caller_a must appear.
        assert!(
            caller_files.contains(&"src/caller_a.py"),
            "caller_a.py calls helper() (imports utils_a), must be a caller. Got: {:?}\nimports: {:?}\ncalls: {:?}",
            caller_files,
            import_resolution,
            call_resolution,
        );

        let imports_resolved = import_resolution
            .iter()
            .any(|(_, r)| r.as_deref() == Some("src/utils_a.py"));
        if imports_resolved {
            assert!(
                !caller_files.contains(&"src/caller_b.py"),
                "caller_b.py imports utils_b, should NOT be a caller of utils_a::helper. Got: {:?}",
                caller_files
            );
        }

        // Ask for callers of utils_b's helper
        let callers = index
            .find_callers("helper", "src/utils_b.py")
            .await
            .unwrap();
        let caller_files: Vec<&str> = callers.iter().map(|(f, _, _, _)| f.as_str()).collect();
        assert!(
            caller_files.contains(&"src/caller_b.py"),
            "caller_b.py calls helper() (imports utils_b), must be a caller. Got: {:?}\nimports: {:?}\ncalls: {:?}",
            caller_files,
            import_resolution,
            call_resolution,
        );
        if imports_resolved {
            assert!(
                !caller_files.contains(&"src/caller_a.py"),
                "caller_a.py imports utils_a, should NOT be a caller of utils_b::helper. Got: {:?}",
                caller_files
            );
        }
    }
}
