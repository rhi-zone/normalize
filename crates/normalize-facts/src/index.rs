use crate::symbols::SymbolParser;
use ignore::WalkBuilder;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use libsql::{Connection, Database, params};
pub use normalize_facts_core::IndexedFile;
use normalize_facts_core::{FlatImport, FlatSymbol, TypeRef};
use normalize_languages::support_for_path;
use normalize_rules_config::WalkConfig;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single CFG block row ready for DB insertion.
struct CfgBlockRow {
    function_qname: String,
    function_start_line: u32,
    block_id: u32,
    kind: String,
    byte_start: usize,
    byte_end: usize,
    start_line: u32,
    end_line: u32,
}

/// A single CFG edge row ready for DB insertion.
struct CfgEdgeRow {
    function_qname: String,
    function_start_line: u32,
    from_block: u32,
    to_block: u32,
    kind: String,
    /// Exception type for EdgeKind::Exception edges (None = conservative).
    exception_type: Option<String>,
}

/// A single CFG def row ready for DB insertion.
struct CfgDefRow {
    function_qname: String,
    function_start_line: u32,
    block_id: u32,
    name: String,
    byte_offset: usize,
    line: u32,
}

/// A single CFG use row ready for DB insertion.
struct CfgUseRow {
    function_qname: String,
    function_start_line: u32,
    block_id: u32,
    name: String,
    byte_offset: usize,
    line: u32,
}

/// A single CFG effect row ready for DB insertion.
struct CfgEffectRow {
    function_qname: String,
    function_start_line: u32,
    block_id: u32,
    kind: String,
    byte_offset: usize,
    line: u32,
    label: Option<String>,
}

/// CFG rows for a single file, ready for DB insertion.
struct CfgData {
    blocks: Vec<CfgBlockRow>,
    edges: Vec<CfgEdgeRow>,
    defs: Vec<CfgDefRow>,
    uses: Vec<CfgUseRow>,
    effects: Vec<CfgEffectRow>,
}

/// A parsed symbol ready for database insertion.
#[derive(serde::Serialize, serde::Deserialize)]
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
    /// CFG data (blocks, edges, defs, uses) for function-level analysis.
    cfg: CfgData,
}

/// CA-cache payload: all extracted data for a single file, keyed by content hash.
/// Does not include `file_path` — that is the lookup key, not part of the payload.
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedFileData {
    symbols: Vec<ParsedSymbol>,
    calls: Vec<CallEntry>,
    imports: Vec<FlatImport>,
    type_methods: Vec<(String, String)>,
    type_refs: Vec<TypeRef>,
}

// Not yet public - just delete .normalize/index.sqlite on schema changes
const SCHEMA_VERSION: i64 = 15;

/// Bump when extraction logic changes to invalidate cached results.
/// Bumped to "2" (2026-04-27): purge CA cache entries that may have been poisoned
/// by the old bug where rebuilds without grammars loaded cached empty results.
const EXTRACTOR_VERSION: &str = "2";

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
    /// Walk configuration — controls which files/directories are visited during
    /// `refresh` and `get_changed_files`. Set via [`FileIndex::set_walk_config`].
    walk_config: WalkConfig,
    /// Content-addressed extraction cache (optional; best-effort).
    ca_cache: Option<crate::ca_cache::CaCache>,
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
                resolved_file TEXT,
                is_reexport INTEGER NOT NULL DEFAULT 0
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
            conn.execute(
                "ALTER TABLE imports ADD COLUMN is_reexport INTEGER NOT NULL DEFAULT 0",
                (),
            )
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
            // CFG tables: clear so next rebuild repopulates them.
            conn.execute("DELETE FROM cfg_blocks", ()).await.ok();
            conn.execute("DELETE FROM cfg_edges", ()).await.ok();
            conn.execute("DELETE FROM cfg_defs", ()).await.ok();
            conn.execute("DELETE FROM cfg_uses", ()).await.ok();
            conn.execute("DELETE FROM cfg_effects", ()).await.ok();
            // Both diagnostic tables get dropped + recreated on every schema bump
            // (column shape has changed in past bumps and may again — simplest path).
            conn.execute("DROP TABLE IF EXISTS daemon_diagnostics", ())
                .await
                .ok();
            conn.execute("DROP TABLE IF EXISTS daemon_diagnostics_per_file", ())
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

        // CFG blocks, edges, defs, and uses for control-flow analysis.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cfg_blocks (
                id INTEGER PRIMARY KEY,
                file TEXT NOT NULL,
                function_qname TEXT NOT NULL,
                function_start_line INTEGER NOT NULL,
                block_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                byte_start INTEGER NOT NULL,
                byte_end INTEGER NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                UNIQUE(file, function_qname, function_start_line, block_id)
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cfg_blocks_file ON cfg_blocks(file)",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cfg_blocks_func ON cfg_blocks(file, function_qname, function_start_line)",
            (),
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cfg_edges (
                id INTEGER PRIMARY KEY,
                file TEXT NOT NULL,
                function_qname TEXT NOT NULL,
                function_start_line INTEGER NOT NULL,
                from_block INTEGER NOT NULL,
                to_block INTEGER NOT NULL,
                kind TEXT NOT NULL,
                exception_type TEXT
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cfg_edges_func ON cfg_edges(file, function_qname, function_start_line)",
            (),
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cfg_defs (
                id INTEGER PRIMARY KEY,
                file TEXT NOT NULL,
                function_qname TEXT NOT NULL,
                function_start_line INTEGER NOT NULL,
                block_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                byte_offset INTEGER NOT NULL,
                line INTEGER NOT NULL
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cfg_defs_func ON cfg_defs(file, function_qname, function_start_line)",
            (),
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cfg_uses (
                id INTEGER PRIMARY KEY,
                file TEXT NOT NULL,
                function_qname TEXT NOT NULL,
                function_start_line INTEGER NOT NULL,
                block_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                byte_offset INTEGER NOT NULL,
                line INTEGER NOT NULL
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cfg_uses_func ON cfg_uses(file, function_qname, function_start_line)",
            (),
        )
        .await?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cfg_effects (
                id INTEGER PRIMARY KEY,
                file TEXT NOT NULL,
                function_qname TEXT NOT NULL,
                function_start_line INTEGER NOT NULL,
                block_id INTEGER NOT NULL,
                kind TEXT NOT NULL,
                byte_offset INTEGER NOT NULL,
                line INTEGER NOT NULL,
                label TEXT
            )",
            (),
        )
        .await?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cfg_effects_func ON cfg_effects(file, function_qname, function_start_line)",
            (),
        )
        .await?;

        // Self-heal the diagnostics tables if a prior daemon left them with a
        // stale column shape (e.g. an interrupted schema migration, or a table
        // created under an older in-version layout before `issues_blob` existed).
        // The `version != SCHEMA_VERSION` block above only drops these on a
        // version *bump*; if the shape drifted without a bump, `CREATE TABLE IF
        // NOT EXISTS` is a no-op and every write fails with
        // "table daemon_diagnostics has no column named issues_blob". Checking
        // the actual columns and dropping on mismatch closes that gap.
        for table in ["daemon_diagnostics", "daemon_diagnostics_per_file"] {
            let mut cols = conn
                .query(&format!("PRAGMA table_info({table})"), ())
                .await?;
            let mut has_issues_blob = false;
            let mut table_exists = false;
            while let Some(row) = cols.next().await? {
                table_exists = true;
                let name: String = row.get(1).unwrap_or_default();
                if name == "issues_blob" {
                    has_issues_blob = true;
                }
            }
            if table_exists && !has_issues_blob {
                conn.execute(&format!("DROP TABLE IF EXISTS {table}"), ())
                    .await
                    .ok();
            }
        }

        // Daemon diagnostics cache: one row per engine. `config_hash` mismatch on load = cache miss.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daemon_diagnostics (
                engine TEXT PRIMARY KEY,
                issues_blob BLOB NOT NULL,
                config_hash TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            (),
        )
        .await?;

        // Per-file diagnostics cache: one row per file that currently has issues.
        // "No row" semantics — files with zero issues are absent from the table.
        // Used by the daemon to serve per-file `RunRules` queries directly without
        // touching the "all" blob.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daemon_diagnostics_per_file (
                path TEXT PRIMARY KEY,
                issues_blob BLOB NOT NULL,
                config_hash TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            (),
        )
        .await?;

        // Open CA cache (best-effort — a failure here is non-fatal)
        let ca_cache = match crate::ca_cache::CaCache::open(
            &crate::ca_cache::CaCache::default_path(),
            1024 * 1024 * 1024, // 1 GiB limit
        ) {
            Ok(c) => {
                // GC stale versions at startup (best-effort)
                if let Err(e) = c.gc_stale_versions(EXTRACTOR_VERSION) {
                    tracing::warn!("normalize-facts: CA cache GC error: {}", e);
                }
                Some(c)
            }
            Err(e) => {
                tracing::warn!("normalize-facts: failed to open CA cache: {}", e);
                None
            }
        };

        Ok(Self {
            conn,
            db,
            root: root.to_path_buf(),
            progress: false,
            walk_config: WalkConfig::default(),
            ca_cache,
        })
    }

    /// Enable progress bar output for long-running operations (refresh, call graph).
    /// Only shows bars when stderr is a terminal.
    pub fn set_progress(&mut self, enabled: bool) {
        self.progress = enabled;
    }

    /// Set the walk configuration used by [`FileIndex::refresh`] and
    /// [`FileIndex::get_changed_files`].
    ///
    /// Call this after `open` to propagate the project's `[walk]` config so the
    /// index walkers respect the same `exclude` patterns as the rest of the system.
    pub fn set_walk_config(&mut self, config: WalkConfig) {
        self.walk_config = config;
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

        // Walk current filesystem using the project's WalkConfig so exclude
        // patterns (e.g. `.normalize/`, `.git/`) are honoured — the same rules
        // the rest of the system uses via gitignore_walk.
        let walk_config = &self.walk_config;
        let ignore_files = walk_config.ignore_files();
        let has_gitignore = ignore_files.contains(&".gitignore");
        let excludes = walk_config.compiled_excludes(&self.root);
        let root_clone = self.root.clone();
        let mut builder = WalkBuilder::new(&self.root);
        builder.hidden(false);
        builder.git_ignore(has_gitignore);
        builder.git_global(has_gitignore);
        builder.git_exclude(has_gitignore);
        for file in &ignore_files {
            if *file != ".gitignore" {
                let ignore_path = self.root.join(file);
                if ignore_path.exists() {
                    builder.add_ignore(ignore_path);
                }
            }
        }
        builder.filter_entry(move |e| {
            let path = e.path();
            let rel = path.strip_prefix(&root_clone).unwrap_or(path);
            if rel.as_os_str().is_empty() {
                return true;
            }
            let is_dir = e.file_type().is_some_and(|ft| ft.is_dir());
            !excludes
                .matched_path_or_any_parents(rel, is_dir)
                .is_ignore()
        });
        let walker = builder.build();

        let mut seen = std::collections::HashSet::new();
        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(&self.root) {
                let rel_str = rel.to_string_lossy().to_string();
                if rel_str.is_empty() {
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

    /// Test/maintenance helper: clear the `last_indexed` meta value so the next
    /// `needs_refresh()` returns `true` regardless of the 60-second debounce.
    ///
    /// Used by integration tests that need to force refresh after each file
    /// edit without waiting for the staleness window.
    pub async fn invalidate_last_indexed(&self) -> Result<(), libsql::Error> {
        self.conn
            .execute("DELETE FROM meta WHERE key = 'last_indexed'", ())
            .await?;
        Ok(())
    }

    /// Refresh only files that have changed (faster than full refresh).
    /// Returns the list of changed file paths (absolute) that were added, modified, or deleted.
    /// The count can be derived from `.len()`.
    pub async fn incremental_refresh(&mut self) -> Result<Vec<PathBuf>, libsql::Error> {
        if !self.needs_refresh().await {
            return Ok(Vec::new());
        }
        self.incremental_refresh_force().await
    }

    /// Begin a write transaction on the persistent connection, first clearing any
    /// transaction that a *previous* call may have left open after erroring out.
    ///
    /// The connection in `FileIndex` is long-lived and reused across every daemon
    /// refresh. If a prior `BEGIN ... COMMIT` block returned early on an error
    /// without rolling back, the transaction stays open and the next bare `BEGIN`
    /// fails with "cannot start a transaction within a transaction" — wedging the
    /// daemon into a state where every subsequent refresh cycle fails. Issuing a
    /// best-effort `ROLLBACK` first guarantees we start from a clean slate even if
    /// some other code path leaked a transaction. Pair with [`commit_or_rollback`].
    async fn begin_clean(&self) -> Result<(), libsql::Error> {
        // ROLLBACK is a no-op (and harmless error) when no transaction is active.
        let _ = self.conn.execute("ROLLBACK", ()).await;
        self.conn.execute("BEGIN", ()).await?;
        Ok(())
    }

    /// Commit a transaction opened by [`begin_clean`] when `body` succeeded, or
    /// roll it back when it failed. Always leaves the connection with no open
    /// transaction so the next [`begin_clean`] is guaranteed to succeed.
    async fn commit_or_rollback(
        &self,
        body: Result<(), libsql::Error>,
    ) -> Result<(), libsql::Error> {
        match body {
            Ok(()) => {
                self.conn.execute("COMMIT", ()).await?;
                Ok(())
            }
            Err(e) => {
                let _ = self.conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }

    /// Refresh only files that have changed, bypassing the `needs_refresh()`
    /// staleness gate.
    ///
    /// `incremental_refresh()` short-circuits if the index was refreshed within
    /// the last 60 seconds and no top-level mtime changes are visible — a cheap
    /// "probably nothing changed" heuristic for cold-CLI callers running many
    /// commands in quick succession. For an event-driven daemon, the watcher
    /// firing **is** the signal that something changed, so the gate is wrong.
    /// Daemons should call this variant.
    pub async fn incremental_refresh_force(&mut self) -> Result<Vec<PathBuf>, libsql::Error> {
        let changed = self.get_changed_files().await?;
        let total_changes = changed.added.len() + changed.modified.len() + changed.deleted.len();

        if total_changes == 0 {
            return Ok(Vec::new());
        }

        self.begin_clean().await?;

        let body: Result<(), libsql::Error> = async {
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
            Ok(())
        }
        .await;
        self.commit_or_rollback(body).await?;

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
        // Build a config-driven walker that respects the project's WalkConfig
        // (same exclude rules as gitignore_walk used everywhere else).
        let ignore_files = self.walk_config.ignore_files();
        let has_gitignore = ignore_files.contains(&".gitignore");
        let excludes = self.walk_config.compiled_excludes(&self.root);
        let root_clone = self.root.clone();
        let mut builder = WalkBuilder::new(&self.root);
        builder.hidden(false);
        builder.git_ignore(has_gitignore);
        builder.git_global(has_gitignore);
        builder.git_exclude(has_gitignore);
        for file in &ignore_files {
            if *file != ".gitignore" {
                let ignore_path = self.root.join(file);
                if ignore_path.exists() {
                    builder.add_ignore(ignore_path);
                }
            }
        }
        builder.filter_entry(move |e| {
            let path = e.path();
            let rel = path.strip_prefix(&root_clone).unwrap_or(path);
            if rel.as_os_str().is_empty() {
                return true;
            }
            let is_dir = e.file_type().is_some_and(|ft| ft.is_dir());
            !excludes
                .matched_path_or_any_parents(rel, is_dir)
                .is_ignore()
        });
        let walker = builder.build();

        self.begin_clean().await?;

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
        let body: Result<(), libsql::Error> = async {
            // Clear existing files
            self.conn.execute("DELETE FROM files", ()).await?;

            for entry in walker.flatten() {
                let path = entry.path();
                if let Ok(rel) = path.strip_prefix(&self.root) {
                    let rel_str = rel.to_string_lossy().to_string();
                    if rel_str.is_empty() {
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
            Ok(())
        }
        .await;
        self.commit_or_rollback(body).await?;

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

    /// Load all resolved import edges with line numbers.
    /// Returns `Vec<(importer_file, line, resolved_file)>` for rows where
    /// `resolved_file IS NOT NULL`. Used by the boundary-violations native rule
    /// to check cross-boundary imports with precise source locations.
    pub async fn all_resolved_imports_with_lines(
        &self,
    ) -> Result<Vec<(String, u32, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, line, resolved_file FROM imports WHERE resolved_file IS NOT NULL",
                (),
            )
            .await?;
        let mut edges = Vec::new();
        while let Some(row) = rows.next().await? {
            let line = u32::try_from(row.get::<i64>(1)?).unwrap_or(0);
            edges.push((row.get(0)?, line, row.get(2)?));
        }
        Ok(edges)
    }

    /// Count distinct resolved import targets per file (fan-out).
    /// Returns `Vec<(file, count)>` ordered by count descending.
    /// Only counts rows where `resolved_file IS NOT NULL`.
    /// Used by the `high-fan-out` native rule.
    pub async fn import_fan_out_by_file(&self) -> Result<Vec<(String, usize)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, COUNT(DISTINCT resolved_file) AS cnt \
                 FROM imports WHERE resolved_file IS NOT NULL \
                 GROUP BY file ORDER BY cnt DESC",
                (),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            let count = usize::try_from(row.get::<i64>(1)?).unwrap_or(0);
            result.push((row.get(0)?, count));
        }
        Ok(result)
    }

    /// Count distinct files that import each file (fan-in).
    /// Returns `Vec<(file, count)>` ordered by count descending.
    /// Only counts rows where `resolved_file IS NOT NULL`.
    /// Used by the `high-fan-in` native rule.
    pub async fn import_fan_in_by_file(&self) -> Result<Vec<(String, usize)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT resolved_file, COUNT(DISTINCT file) AS cnt \
                 FROM imports WHERE resolved_file IS NOT NULL \
                 GROUP BY resolved_file ORDER BY cnt DESC",
                (),
            )
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            let count = usize::try_from(row.get::<i64>(1)?).unwrap_or(0);
            result.push((row.get(0)?, count));
        }
        Ok(result)
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

    /// Find the shortest import path(s) from `from` to `to` via BFS over the resolved import graph.
    ///
    /// `from` and `to` are root-relative path strings (as stored in the DB).
    /// Returns all shortest paths (there may be more than one of equal length).
    /// If `all_paths` is true, returns all simple paths up to `path_limit` paths
    /// and up to `max_depth` hops deep.
    /// Returns an empty vec if no path exists.
    pub async fn find_import_path(
        &self,
        from: &str,
        to: &str,
        all_paths: bool,
        path_limit: usize,
        max_depth: usize,
    ) -> Result<Vec<Vec<String>>, libsql::Error> {
        use std::collections::{HashMap, HashSet, VecDeque};

        if from == to {
            return Ok(vec![vec![from.to_string()]]);
        }

        // Build adjacency list: file -> set of files it imports
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        let mut rows = self
            .conn
            .query(
                "SELECT file, resolved_file FROM imports WHERE resolved_file IS NOT NULL",
                (),
            )
            .await?;
        while let Some(row) = rows.next().await? {
            let file: String = row.get(0)?;
            let resolved: String = row.get(1)?;
            adj.entry(file).or_default().push(resolved);
        }

        if !all_paths {
            // BFS for shortest path
            let mut visited: HashMap<String, String> = HashMap::new(); // node -> parent
            let mut queue: VecDeque<String> = VecDeque::new();
            queue.push_back(from.to_string());
            visited.insert(from.to_string(), String::new());

            let mut found = false;
            'bfs: while let Some(node) = queue.pop_front() {
                // Check depth
                let depth = {
                    let mut d = 0usize;
                    let mut cur = &node;
                    while let Some(p) = visited.get(cur) {
                        if p.is_empty() {
                            break;
                        }
                        d += 1;
                        cur = p;
                        if d > max_depth {
                            break;
                        }
                    }
                    d
                };
                if depth >= max_depth {
                    continue;
                }
                if let Some(neighbors) = adj.get(&node) {
                    for neighbor in neighbors {
                        if !visited.contains_key(neighbor.as_str()) {
                            visited.insert(neighbor.clone(), node.clone());
                            if neighbor == to {
                                found = true;
                                break 'bfs;
                            }
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }

            if !found {
                return Ok(vec![]);
            }

            // Reconstruct path by backtracking through visited
            let mut path = vec![to.to_string()];
            let mut cur = to.to_string();
            loop {
                let parent = visited.get(&cur).cloned().unwrap_or_default();
                if parent.is_empty() {
                    break;
                }
                path.push(parent.clone());
                cur = parent;
            }
            path.reverse();
            Ok(vec![path])
        } else {
            // DFS to find all simple paths up to path_limit
            let mut result: Vec<Vec<String>> = Vec::new();
            let mut stack: VecDeque<(String, Vec<String>, HashSet<String>)> = VecDeque::new();
            let mut initial_visited = HashSet::new();
            initial_visited.insert(from.to_string());
            stack.push_back((from.to_string(), vec![from.to_string()], initial_visited));

            while let Some((node, path, visited)) = stack.pop_back() {
                if result.len() >= path_limit {
                    break;
                }
                if path.len() > max_depth + 1 {
                    continue;
                }
                if let Some(neighbors) = adj.get(&node) {
                    for neighbor in neighbors {
                        if visited.contains(neighbor.as_str()) {
                            continue;
                        }
                        let mut new_path = path.clone();
                        new_path.push(neighbor.clone());
                        if neighbor == to {
                            result.push(new_path);
                            if result.len() >= path_limit {
                                break;
                            }
                        } else {
                            let mut new_visited = visited.clone();
                            new_visited.insert(neighbor.clone());
                            stack.push_back((neighbor.clone(), new_path, new_visited));
                        }
                    }
                }
            }

            Ok(result)
        }
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

    /// Return all CFG effect rows: (file, function_qname, function_start_line, block_id, kind, line, label).
    /// Query all CFG edge facts from the index.
    /// Returns tuples of (file, function_qname, function_start_line, from_block, to_block, kind, exception_type).
    pub async fn all_cfg_edges(
        &self,
    ) -> Result<Vec<(String, String, u32, u32, u32, String, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, function_qname, function_start_line, from_block, to_block, kind, COALESCE(exception_type, '') FROM cfg_edges",
                (),
            )
            .await?;
        let mut edges = Vec::new();
        while let Some(row) = rows.next().await? {
            edges.push((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                u32::try_from(row.get::<i64>(2)?).unwrap_or(0),
                u32::try_from(row.get::<i64>(3)?).unwrap_or(0),
                u32::try_from(row.get::<i64>(4)?).unwrap_or(0),
                row.get::<String>(5)?,
                row.get::<String>(6)?,
            ));
        }
        Ok(edges)
    }

    pub async fn all_cfg_effects(
        &self,
    ) -> Result<Vec<(String, String, u32, u32, String, u32, String)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, function_qname, function_start_line, block_id, kind, line, COALESCE(label, '') FROM cfg_effects",
                (),
            )
            .await?;
        let mut effects = Vec::new();
        while let Some(row) = rows.next().await? {
            effects.push((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                u32::try_from(row.get::<i64>(2)?).unwrap_or(0),
                u32::try_from(row.get::<i64>(3)?).unwrap_or(0),
                row.get::<String>(4)?,
                u32::try_from(row.get::<i64>(5)?).unwrap_or(0),
                row.get::<String>(6)?,
            ));
        }
        Ok(effects)
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

    /// Resolve import specifiers using per-language `ModuleResolver` implementations.
    ///
    /// Runs after `resolve_all_imports` as a second pass: for any import row that still
    /// has `resolved_file IS NULL`, look up the language's `ModuleResolver` and call
    /// `resolve()` directly. Updates `resolved_file` on successful resolutions.
    ///
    /// Uses the workspace root to build `ResolverConfig` once per language, then
    /// resolves all pending imports for that language's files.
    pub async fn resolve_imports_via_module_resolver(&self) -> Result<usize, libsql::Error> {
        use normalize_languages::{ImportSpec, Resolution, support_for_path};
        use std::collections::HashMap;

        // Collect pending imports: (file, module, name)
        let mut rows = self
            .conn
            .query(
                "SELECT file, module, name FROM imports WHERE module IS NOT NULL AND resolved_file IS NULL",
                (),
            )
            .await?;
        let mut pending: Vec<(String, String, String)> = Vec::new();
        while let Some(row) = rows.next().await? {
            let module: Option<String> = row.get(1)?;
            if let Some(module) = module {
                pending.push((row.get(0)?, module, row.get(2)?));
            }
        }

        if pending.is_empty() {
            return Ok(0);
        }

        // Build resolver configs keyed by language name (cache per workspace)
        let mut resolver_configs: HashMap<&'static str, normalize_languages::ResolverConfig> =
            HashMap::new();

        let mut resolved_count = 0usize;
        for (file_str, module_str, name_str) in &pending {
            let file_path = self.root.join(file_str);
            let lang = match support_for_path(&file_path) {
                Some(l) => l,
                None => continue,
            };
            let resolver = match lang.module_resolver() {
                Some(r) => r,
                None => continue,
            };

            let cfg = resolver_configs
                .entry(lang.name())
                .or_insert_with(|| resolver.workspace_config(&self.root));

            let spec = ImportSpec {
                raw: module_str.clone(),
                is_relative: module_str.starts_with('.'),
                names: if name_str == "*" {
                    Vec::new()
                } else {
                    vec![name_str.clone()]
                },
                is_glob: name_str == "*",
            };

            if let Resolution::Resolved(resolved_path, _) = resolver.resolve(&file_path, &spec, cfg)
            {
                // Convert absolute resolved path to root-relative string
                let resolved_rel = resolved_path
                    .strip_prefix(&self.root)
                    .unwrap_or(&resolved_path)
                    .to_string_lossy()
                    .to_string();

                self.conn
                    .execute(
                        "UPDATE imports SET resolved_file = ?1 WHERE file = ?2 AND module = ?3 AND name = ?4 AND resolved_file IS NULL",
                        libsql::params![resolved_rel, file_str.clone(), module_str.clone(), name_str.clone()],
                    )
                    .await?;
                resolved_count += 1;
            }
        }

        Ok(resolved_count)
    }

    /// Follow re-export chains to resolve imports to their ultimate source file.
    ///
    /// When file A imports `Foo` from file B, but file B re-exports `Foo` from file C
    /// (via `pub use c::Foo` in Rust or `export { Foo } from './c'` in TypeScript),
    /// this updates A's import row so `resolved_file` points to C instead of B.
    ///
    /// Runs iteratively (up to `max_depth` passes) to handle chains longer than one hop,
    /// stopping early when no rows are updated. Wildcard re-exports (`pub use mod::*`)
    /// are handled by following any re-export from the intermediate file.
    pub async fn trace_reexports(&self) -> Result<usize, libsql::Error> {
        let max_depth = 10usize;
        let mut total_updated = 0usize;

        for _ in 0..max_depth {
            // For each import row whose resolved_file re-exports the imported name
            // (or re-exports via wildcard), update resolved_file to point to the
            // re-export's own resolved_file (the ultimate source).
            //
            // A re-export in file B for name N means: imports row where
            //   file = B, name = N (or name = '*'), is_reexport = 1, resolved_file IS NOT NULL
            //
            // We look for imports in A where:
            //   resolved_file = B  AND  B has a matching re-export row with its own resolved_file
            let updated = self
                .conn
                .execute(
                    "UPDATE imports AS consumer
                     SET resolved_file = (
                         SELECT reexp.resolved_file
                         FROM imports AS reexp
                         WHERE reexp.file = consumer.resolved_file
                           AND reexp.is_reexport = 1
                           AND reexp.resolved_file IS NOT NULL
                           AND reexp.resolved_file != consumer.resolved_file
                           AND (
                               reexp.name = consumer.name
                               OR COALESCE(reexp.alias, reexp.name) = consumer.name
                               OR reexp.name = '*'
                           )
                         LIMIT 1
                     )
                     WHERE consumer.resolved_file IS NOT NULL
                       AND EXISTS (
                           SELECT 1 FROM imports AS reexp2
                           WHERE reexp2.file = consumer.resolved_file
                             AND reexp2.is_reexport = 1
                             AND reexp2.resolved_file IS NOT NULL
                             AND reexp2.resolved_file != consumer.resolved_file
                             AND (
                                 reexp2.name = consumer.name
                                 OR COALESCE(reexp2.alias, reexp2.name) = consumer.name
                                 OR reexp2.name = '*'
                             )
                       )",
                    (),
                )
                .await? as usize;

            total_updated += updated;
            if updated == 0 {
                break;
            }
        }

        Ok(total_updated)
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

    /// Find files that import a specific symbol by name, including the module path.
    /// Returns: (file, imported_name, alias, line, module)
    /// Useful for `move`: the recipe needs the original module string so it can rewrite
    /// it to the new path verbatim, rather than guessing where the path begins/ends.
    pub async fn find_symbol_importers_with_module(
        &self,
        symbol_name: &str,
    ) -> Result<Vec<(String, String, Option<String>, usize, Option<String>)>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT file, name, alias, line, module FROM imports WHERE name = ?1",
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
                row.get(4)?,
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

        // Pre-pass: check CA cache for all files (serial, fast disk reads)
        let mut cached_data: Vec<ParsedFileData> = Vec::new();
        let mut uncached_files: Vec<String> = Vec::new();
        // Files whose symbol data came from CA cache: need CFG rebuilt separately.
        let mut ca_cached_files: Vec<String> = Vec::new();

        for file_path in &files {
            let full_path = root.join(file_path);
            let bytes = match std::fs::read(&full_path) {
                Ok(b) => b,
                Err(_) => {
                    uncached_files.push(file_path.clone());
                    continue;
                }
            };
            let grammar = match support_for_path(&full_path) {
                Some(s) => s.grammar_name().to_string(),
                None => {
                    uncached_files.push(file_path.clone());
                    continue;
                }
            };
            let hash = blake3::hash(&bytes);
            if let Some(ca) = &self.ca_cache {
                match ca.get::<CachedFileData>(hash.as_bytes(), EXTRACTOR_VERSION, &grammar) {
                    Ok(Some(cached)) => {
                        ca_cached_files.push(file_path.clone());
                        cached_data.push(ParsedFileData {
                            file_path: file_path.clone(),
                            symbols: cached.symbols,
                            calls: cached.calls,
                            imports: cached.imports,
                            type_methods: cached.type_methods,
                            type_refs: cached.type_refs,
                            // CFG data is not CA-cached — always rebuilt during parse.
                            cfg: CfgData {
                                blocks: Vec::new(),
                                edges: Vec::new(),
                                defs: Vec::new(),
                                uses: Vec::new(),
                                effects: Vec::new(),
                            },
                        });
                        continue;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!("normalize-facts: CA cache get error: {}", e);
                    }
                }
            }
            uncached_files.push(file_path.clone());
        }

        let ca_cache_for_rayon = self.ca_cache.clone();

        let pb = if self.progress && std::io::IsTerminal::is_terminal(&std::io::stderr()) {
            let pb = ProgressBar::new(uncached_files.len() as u64);
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
        let mut parsed_data: Vec<ParsedFileData> = uncached_files
            .par_iter()
            .progress_with(pb.clone())
            .filter_map(|file_path| {
                let full_path = root.join(file_path);
                let bytes = std::fs::read(&full_path).ok()?;
                let content = String::from_utf8_lossy(&bytes).into_owned();

                let grammar = support_for_path(&full_path)
                    .map(|s| s.grammar_name().to_string())
                    .unwrap_or_default();
                let hash = blake3::hash(&bytes);

                // Each thread creates its own parser
                let mut parser = SymbolParser::new();

                // parse_file returns None when the grammar .so is unavailable.
                // In that case, skip the file entirely — don't index it as empty.
                // The missing grammar is already recorded in `parsers::report_missing_grammar`
                // (called from `parse_file` -> `try_get_grammar`), so callers can summarise.
                let symbols = parser.parse_file(&full_path, &content)?;

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

                // Build CFGs for function/method symbols (best-effort — errors are non-fatal).
                let cfg = build_cfg_data_for_file(&full_path, &bytes, grammar.as_str(), &symbols);

                // Store result in CA cache (best-effort).
                // Grammar availability is already guaranteed above (parse_file returned Some),
                // so empty results here are legitimate and safe to cache.
                if !grammar.is_empty()
                    && let Some(ca) = &ca_cache_for_rayon
                {
                    let cached = CachedFileData {
                        symbols: sym_data
                            .iter()
                            .map(|s| ParsedSymbol {
                                name: s.name.clone(),
                                kind: s.kind.clone(),
                                start_line: s.start_line,
                                end_line: s.end_line,
                                parent: s.parent.clone(),
                                visibility: s.visibility.clone(),
                                attributes: s.attributes.clone(),
                                is_interface_impl: s.is_interface_impl,
                                implements: s.implements.clone(),
                                docstring: s.docstring.clone(),
                            })
                            .collect(),
                        calls: call_data.clone(),
                        imports: imports.clone(),
                        type_methods: type_methods.clone(),
                        type_refs: type_refs.clone(),
                    };
                    if let Err(e) = ca.put(hash.as_bytes(), EXTRACTOR_VERSION, &grammar, &cached) {
                        tracing::warn!("normalize-facts: CA cache put error: {}", e);
                    }
                }

                Some(ParsedFileData {
                    file_path: file_path.clone(),
                    symbols: sym_data,
                    calls: call_data,
                    imports,
                    type_methods,
                    type_refs,
                    cfg,
                })
            })
            .collect();

        // Merge CA-cached results
        parsed_data.extend(cached_data);

        // Build CFG data for files that came from the CA cache (their cfg vecs are empty).
        if !ca_cached_files.is_empty() {
            // For each cached file, rebuild CFG data using a fresh parser (re-reads the file).
            let cfg_updates: Vec<(String, CfgData)> = ca_cached_files
                .par_iter()
                .filter_map(|file_path| {
                    let full_path = root.join(file_path);
                    let bytes = std::fs::read(&full_path).ok()?;
                    let lang_support = support_for_path(&full_path)?;
                    let grammar_name = lang_support.grammar_name();
                    let symbols: Vec<FlatSymbol> = {
                        let p = SymbolParser::new();
                        let content = String::from_utf8_lossy(&bytes).into_owned();
                        p.parse_file(&full_path, &content)?
                    };
                    let cfg = build_cfg_data_for_file(&full_path, &bytes, grammar_name, &symbols);
                    Some((file_path.clone(), cfg))
                })
                .collect();
            // Patch cfg into parsed_data.
            for (fpath, cfg) in cfg_updates {
                if let Some(data) = parsed_data.iter_mut().find(|d| d.file_path == fpath) {
                    data.cfg = cfg;
                }
            }
        }

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

        self.begin_clean().await?;

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
        self.conn.execute("DELETE FROM cfg_blocks", ()).await?;
        self.conn.execute("DELETE FROM cfg_edges", ()).await?;
        self.conn.execute("DELETE FROM cfg_defs", ()).await?;
        self.conn.execute("DELETE FROM cfg_uses", ()).await?;
        self.conn.execute("DELETE FROM cfg_effects", ()).await?;

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
                    "INSERT INTO imports (file, module, name, alias, line, is_reexport) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![data.file_path.clone(), imp.module.clone(), imp.name.clone(), imp.alias.clone(), imp.line as i64, imp.is_reexport as i64],
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

            // Insert CFG blocks
            for blk in &data.cfg.blocks {
                self.conn.execute(
                    "INSERT OR IGNORE INTO cfg_blocks (file, function_qname, function_start_line, block_id, kind, byte_start, byte_end, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        data.file_path.clone(),
                        blk.function_qname.clone(),
                        blk.function_start_line as i64,
                        blk.block_id as i64,
                        blk.kind.clone(),
                        blk.byte_start as i64,
                        blk.byte_end as i64,
                        blk.start_line as i64,
                        blk.end_line as i64,
                    ],
                ).await?;
            }
            // Insert CFG edges
            for edge in &data.cfg.edges {
                self.conn.execute(
                    "INSERT INTO cfg_edges (file, function_qname, function_start_line, from_block, to_block, kind, exception_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        data.file_path.clone(),
                        edge.function_qname.clone(),
                        edge.function_start_line as i64,
                        edge.from_block as i64,
                        edge.to_block as i64,
                        edge.kind.clone(),
                        edge.exception_type.clone(),
                    ],
                ).await?;
            }
            // Insert CFG defs
            for def in &data.cfg.defs {
                self.conn.execute(
                    "INSERT INTO cfg_defs (file, function_qname, function_start_line, block_id, name, byte_offset, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        data.file_path.clone(),
                        def.function_qname.clone(),
                        def.function_start_line as i64,
                        def.block_id as i64,
                        def.name.clone(),
                        def.byte_offset as i64,
                        def.line as i64,
                    ],
                ).await?;
            }
            // Insert CFG uses
            for use_ in &data.cfg.uses {
                self.conn.execute(
                    "INSERT INTO cfg_uses (file, function_qname, function_start_line, block_id, name, byte_offset, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        data.file_path.clone(),
                        use_.function_qname.clone(),
                        use_.function_start_line as i64,
                        use_.block_id as i64,
                        use_.name.clone(),
                        use_.byte_offset as i64,
                        use_.line as i64,
                    ],
                ).await?;
            }
            // Insert CFG effects
            for eff in &data.cfg.effects {
                self.conn.execute(
                    "INSERT INTO cfg_effects (file, function_qname, function_start_line, block_id, kind, byte_offset, line, label) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        data.file_path.clone(),
                        eff.function_qname.clone(),
                        eff.function_start_line as i64,
                        eff.block_id as i64,
                        eff.kind.clone(),
                        eff.byte_offset as i64,
                        eff.line as i64,
                        eff.label.clone(),
                    ],
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
        // Second pass: use per-language ModuleResolver for remaining unresolved imports.
        self.resolve_imports_via_module_resolver()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "normalize-facts: resolve_imports_via_module_resolver error: {}",
                    e
                );
                0
            });
        // Follow re-export chains so imports resolve to ultimate source files.
        self.trace_reexports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: trace_reexports error: {}", e);
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
            self.conn
                .execute(
                    "DELETE FROM cfg_blocks WHERE file = ?1",
                    params![path.clone()],
                )
                .await?;
            self.conn
                .execute(
                    "DELETE FROM cfg_edges WHERE file = ?1",
                    params![path.clone()],
                )
                .await?;
            self.conn
                .execute(
                    "DELETE FROM cfg_defs WHERE file = ?1",
                    params![path.clone()],
                )
                .await?;
            self.conn
                .execute(
                    "DELETE FROM cfg_uses WHERE file = ?1",
                    params![path.clone()],
                )
                .await?;
            self.conn
                .execute(
                    "DELETE FROM cfg_effects WHERE file = ?1",
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
            let bytes = match std::fs::read(&full_path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let grammar = support_for_path(&full_path)
                .map(|s| s.grammar_name().to_string())
                .unwrap_or_default();
            let hash = blake3::hash(&bytes);

            // Try CA cache first (best-effort)
            let cached: Option<CachedFileData> = if !grammar.is_empty() {
                self.ca_cache.as_ref().and_then(|ca| {
                    ca.get::<CachedFileData>(hash.as_bytes(), EXTRACTOR_VERSION, &grammar)
                        .unwrap_or_else(|e| {
                            tracing::warn!("normalize-facts: CA cache get error: {}", e);
                            None
                        })
                })
            } else {
                None
            };

            let (sym_data, call_data, imports, type_refs) = if let Some(c) = cached {
                (c.symbols, c.calls, c.imports, c.type_refs)
            } else {
                let content = String::from_utf8_lossy(&bytes).into_owned();

                // parse_file returns None when the grammar .so is unavailable.
                // Skip the file entirely — don't index it as empty.
                // The missing grammar is already recorded in `parsers::report_missing_grammar`
                // (called from `parse_file` -> `try_get_grammar`), so callers can summarise.
                let symbols = match parser.parse_file(&full_path, &content) {
                    Some(s) => s,
                    None => continue,
                };

                let mut sym_data = Vec::with_capacity(symbols.len());
                let mut call_data_local: Vec<CallEntry> = Vec::new();

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
                    let kind = sym.kind.as_str();
                    if kind == "function" || kind == "method" {
                        let calls = parser.find_callees_for_symbol(&full_path, &content, sym);
                        for (callee_name, line, qualifier, access) in calls {
                            call_data_local.push((
                                sym.name.clone(),
                                callee_name,
                                qualifier,
                                access,
                                line,
                            ));
                        }
                    }
                }

                let imports = parser.parse_imports(&full_path, &content);
                let type_refs = parser.find_type_refs(&full_path, &content);

                // Store in CA cache (best-effort).
                // Grammar availability is already guaranteed above (parse_file returned Some),
                // so empty results here are legitimate and safe to cache.
                if !grammar.is_empty()
                    && let Some(ca) = &self.ca_cache
                {
                    let cached_store = CachedFileData {
                        symbols: sym_data
                            .iter()
                            .map(|s| ParsedSymbol {
                                name: s.name.clone(),
                                kind: s.kind.clone(),
                                start_line: s.start_line,
                                end_line: s.end_line,
                                parent: s.parent.clone(),
                                visibility: s.visibility.clone(),
                                attributes: s.attributes.clone(),
                                is_interface_impl: s.is_interface_impl,
                                implements: s.implements.clone(),
                                docstring: s.docstring.clone(),
                            })
                            .collect(),
                        calls: call_data_local.clone(),
                        imports: imports.clone(),
                        type_methods: Vec::new(), // type_methods not extracted in incremental path
                        type_refs: type_refs.clone(),
                    };
                    if let Err(e) =
                        ca.put(hash.as_bytes(), EXTRACTOR_VERSION, &grammar, &cached_store)
                    {
                        tracing::warn!("normalize-facts: CA cache put error: {}", e);
                    }
                }

                (sym_data, call_data_local, imports, type_refs)
            };

            // Insert symbols
            for sym in &sym_data {
                self.conn.execute(
                    "INSERT INTO symbols (file, name, kind, start_line, end_line, parent, visibility, is_impl) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![file_path.clone(), sym.name.clone(), sym.kind.clone(), sym.start_line as i64, sym.end_line as i64, sym.parent.clone(), sym.visibility.clone(), sym.is_interface_impl as i64],
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
            }

            // Insert calls
            for (caller_symbol, callee_name, qualifier, access, line) in &call_data {
                self.conn.execute(
                    "INSERT INTO calls (caller_file, caller_symbol, callee_name, callee_qualifier, access, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![file_path.clone(), caller_symbol.clone(), callee_name.clone(), qualifier.clone(), access.clone(), *line as i64],
                ).await?;
                call_count += 1;
            }

            // Insert imports
            for imp in &imports {
                self.conn.execute(
                    "INSERT INTO imports (file, module, name, alias, line, is_reexport) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![file_path.clone(), imp.module.clone(), imp.name.clone(), imp.alias.clone(), imp.line as i64, imp.is_reexport as i64],
                ).await?;
                import_count += 1;
            }

            // Insert type references
            for tr in &type_refs {
                self.conn.execute(
                    "INSERT INTO type_refs (file, source_symbol, target_type, kind, line) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![file_path.clone(), tr.source_symbol.clone(), tr.target_type.clone(), tr.kind.as_str(), tr.line as i64],
                ).await?;
            }

            // Build and insert CFG data (best-effort).
            let full_path_for_cfg = self.root.join(file_path);
            let grammar_for_cfg = support_for_path(&full_path_for_cfg)
                .map(|s| s.grammar_name().to_string())
                .unwrap_or_default();
            if !grammar_for_cfg.is_empty() {
                // Parse FlatSymbol list to get function symbols (needed for CFG building).
                let flat_symbols: Vec<FlatSymbol> = {
                    let p = SymbolParser::new();
                    let content = String::from_utf8_lossy(&bytes).into_owned();
                    p.parse_file(&full_path_for_cfg, &content)
                        .unwrap_or_default()
                };
                let cfg_data = build_cfg_data_for_file(
                    &full_path_for_cfg,
                    &bytes,
                    &grammar_for_cfg,
                    &flat_symbols,
                );
                for blk in &cfg_data.blocks {
                    self.conn.execute(
                        "INSERT OR IGNORE INTO cfg_blocks (file, function_qname, function_start_line, block_id, kind, byte_start, byte_end, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        params![
                            file_path.clone(),
                            blk.function_qname.clone(),
                            blk.function_start_line as i64,
                            blk.block_id as i64,
                            blk.kind.clone(),
                            blk.byte_start as i64,
                            blk.byte_end as i64,
                            blk.start_line as i64,
                            blk.end_line as i64,
                        ],
                    ).await?;
                }
                for edge in &cfg_data.edges {
                    self.conn.execute(
                        "INSERT INTO cfg_edges (file, function_qname, function_start_line, from_block, to_block, kind, exception_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            file_path.clone(),
                            edge.function_qname.clone(),
                            edge.function_start_line as i64,
                            edge.from_block as i64,
                            edge.to_block as i64,
                            edge.kind.clone(),
                            edge.exception_type.clone(),
                        ],
                    ).await?;
                }
                for def in &cfg_data.defs {
                    self.conn.execute(
                        "INSERT INTO cfg_defs (file, function_qname, function_start_line, block_id, name, byte_offset, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            file_path.clone(),
                            def.function_qname.clone(),
                            def.function_start_line as i64,
                            def.block_id as i64,
                            def.name.clone(),
                            def.byte_offset as i64,
                            def.line as i64,
                        ],
                    ).await?;
                }
                for use_ in &cfg_data.uses {
                    self.conn.execute(
                        "INSERT INTO cfg_uses (file, function_qname, function_start_line, block_id, name, byte_offset, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            file_path.clone(),
                            use_.function_qname.clone(),
                            use_.function_start_line as i64,
                            use_.block_id as i64,
                            use_.name.clone(),
                            use_.byte_offset as i64,
                            use_.line as i64,
                        ],
                    ).await?;
                }
                for eff in &cfg_data.effects {
                    self.conn.execute(
                        "INSERT INTO cfg_effects (file, function_qname, function_start_line, block_id, kind, byte_offset, line, label) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            file_path.clone(),
                            eff.function_qname.clone(),
                            eff.function_start_line as i64,
                            eff.block_id as i64,
                            eff.kind.clone(),
                            eff.byte_offset as i64,
                            eff.line as i64,
                            eff.label.clone(),
                        ],
                    ).await?;
                }
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
            .chain(changed.modified)
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

        self.begin_clean().await?;
        let stats = match self
            .reindex_files(&deleted_source_files, &changed_files)
            .await
        {
            Ok(stats) => {
                self.conn.execute("COMMIT", ()).await?;
                stats
            }
            Err(e) => {
                let _ = self.conn.execute("ROLLBACK", ()).await;
                return Err(e);
            }
        };

        // Resolve any newly inserted imports to root-relative file paths.
        self.resolve_all_imports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_imports error: {}", e);
            0
        });
        // Second pass: use per-language ModuleResolver for remaining unresolved imports.
        self.resolve_imports_via_module_resolver()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "normalize-facts: resolve_imports_via_module_resolver error: {}",
                    e
                );
                0
            });
        // Follow re-export chains so imports resolve to ultimate source files.
        self.trace_reexports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: trace_reexports error: {}", e);
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

        self.begin_clean().await?;
        let reindex_result = if exists {
            self.reindex_files(&[], &[rel_path.to_string()]).await
        } else {
            self.reindex_files(&[rel_path.to_string()], &[]).await
        };
        let stats = match reindex_result {
            Ok(stats) => {
                self.conn.execute("COMMIT", ()).await?;
                stats
            }
            Err(e) => {
                let _ = self.conn.execute("ROLLBACK", ()).await;
                return Err(e);
            }
        };

        self.resolve_all_imports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: resolve_all_imports error: {}", e);
            0
        });
        self.resolve_imports_via_module_resolver()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "normalize-facts: resolve_imports_via_module_resolver error: {}",
                    e
                );
                0
            });
        self.trace_reexports().await.unwrap_or_else(|e| {
            tracing::warn!("normalize-facts: trace_reexports error: {}", e);
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

    /// Persist rkyv-serialized diagnostics blob for one engine ("syntax", "fact", "native", "all").
    /// Replaces any previous value for that engine.
    ///
    /// `config_hash` is stamped on the row so callers can detect blobs produced
    /// under a different config (cross-daemon-restart staleness). See
    /// `load_diagnostics_blob` for the matching read side.
    pub async fn save_diagnostics_blob(
        &self,
        engine: &str,
        blob: &[u8],
        config_hash: &str,
    ) -> Result<(), libsql::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO daemon_diagnostics (engine, issues_blob, config_hash, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![engine.to_string(), blob.to_vec(), config_hash.to_string(), now],
            )
            .await?;
        Ok(())
    }

    /// Load rkyv-serialized diagnostics blob for one engine.
    ///
    /// Returns `None` if no row exists *or* the row's `config_hash` does not
    /// match `expected_hash`. The mismatch case is treated as a cache miss so
    /// the caller will reprime under the current config rather than serving a
    /// blob from a previous daemon session.
    pub async fn load_diagnostics_blob(
        &self,
        engine: &str,
        expected_hash: &str,
    ) -> Result<Option<Vec<u8>>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT issues_blob, config_hash FROM daemon_diagnostics WHERE engine = ?1",
                params![engine.to_string()],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let blob: Vec<u8> = row.get(0)?;
            let stored_hash: String = row.get(1)?;
            if stored_hash == expected_hash {
                Ok(Some(blob))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Replace per-file diagnostics blobs in a single transaction.
    ///
    /// `upserts`: `(relative_path, rkyv_blob)` — files that have issues.
    /// `deletes`: relative paths that became clean (had a row, now don't).
    ///
    /// All upserts and deletes commit atomically so readers never see a
    /// partially-updated state.
    pub async fn save_diagnostics_per_file(
        &self,
        upserts: &[(String, Vec<u8>)],
        deletes: &[String],
        config_hash: &str,
    ) -> Result<(), libsql::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.begin_clean().await?;
        let result: Result<(), libsql::Error> = async {
            for (path, blob) in upserts {
                self.conn
                    .execute(
                        "INSERT OR REPLACE INTO daemon_diagnostics_per_file
                         (path, issues_blob, config_hash, updated_at) VALUES (?1, ?2, ?3, ?4)",
                        params![path.clone(), blob.clone(), config_hash.to_string(), now],
                    )
                    .await?;
            }
            for path in deletes {
                self.conn
                    .execute(
                        "DELETE FROM daemon_diagnostics_per_file WHERE path = ?1",
                        params![path.clone()],
                    )
                    .await?;
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {
                self.conn.execute("COMMIT", ()).await?;
                Ok(())
            }
            Err(e) => {
                let _ = self.conn.execute("ROLLBACK", ()).await;
                Err(e)
            }
        }
    }

    /// Load the rkyv blob for one file. `None` = no row (file is clean) or the
    /// row's `config_hash` doesn't match `expected_hash` (stale across config
    /// change).
    pub async fn load_diagnostics_for_file(
        &self,
        path: &str,
        expected_hash: &str,
    ) -> Result<Option<Vec<u8>>, libsql::Error> {
        let mut rows = self
            .conn
            .query(
                "SELECT issues_blob, config_hash FROM daemon_diagnostics_per_file WHERE path = ?1",
                params![path.to_string()],
            )
            .await?;
        if let Some(row) = rows.next().await? {
            let blob: Vec<u8> = row.get(0)?;
            let stored_hash: String = row.get(1)?;
            if stored_hash == expected_hash {
                Ok(Some(blob))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Load blobs for many files. Skips files with no row or whose stored
    /// `config_hash` doesn't match `expected_hash`.
    /// Returns `(path, blob)` pairs in arbitrary order.
    pub async fn load_diagnostics_for_files(
        &self,
        paths: &[String],
        expected_hash: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, libsql::Error> {
        let mut out = Vec::new();
        for path in paths {
            let mut rows = self
                .conn
                .query(
                    "SELECT path, issues_blob, config_hash FROM daemon_diagnostics_per_file WHERE path = ?1",
                    params![path.clone()],
                )
                .await?;
            if let Some(row) = rows.next().await? {
                let p: String = row.get(0)?;
                let b: Vec<u8> = row.get(1)?;
                let stored_hash: String = row.get(2)?;
                if stored_hash == expected_hash {
                    out.push((p, b));
                }
            }
        }
        Ok(out)
    }

    /// Drop every cached diagnostic row (both per-engine blobs and the
    /// per-file table). Used by the daemon when `.normalize/config.toml` or a
    /// rule-definition file changes — the cached blobs reflect the *previous*
    /// config, so they must be cleared before a full reprime to prevent stale
    /// `RunRules` results being served between the config change and the
    /// reprime completing.
    pub async fn clear_all_diagnostics(&self) -> Result<(), libsql::Error> {
        self.conn
            .execute("DELETE FROM daemon_diagnostics", ())
            .await?;
        self.conn
            .execute("DELETE FROM daemon_diagnostics_per_file", ())
            .await?;
        Ok(())
    }

    /// Return all paths that currently have a per-file diagnostics row.
    /// Used by the daemon refresh diff to detect files that became clean.
    pub async fn list_diagnostic_paths(&self) -> Result<Vec<String>, libsql::Error> {
        let mut rows = self
            .conn
            .query("SELECT path FROM daemon_diagnostics_per_file", ())
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(row.get(0)?);
        }
        Ok(out)
    }
}

// =============================================================================
// CFG building helpers
// =============================================================================

/// Build CFG data (blocks, edges, defs, uses) for all function/method symbols in a file.
///
/// Returns four vecs of rows ready for DB insertion. Errors from individual function builds
/// are silently ignored (best-effort) so a broken CFG query doesn't abort the whole index.
fn build_cfg_data_for_file(
    full_path: &Path,
    source_bytes: &[u8],
    grammar_name: &str,
    symbols: &[FlatSymbol],
) -> CfgData {
    let mut all_blocks: Vec<CfgBlockRow> = Vec::new();
    let mut all_edges: Vec<CfgEdgeRow> = Vec::new();
    let mut all_defs: Vec<CfgDefRow> = Vec::new();
    let mut all_uses: Vec<CfgUseRow> = Vec::new();
    let mut all_effects: Vec<CfgEffectRow> = Vec::new();

    // Helper macro to construct an early-return CfgData.
    macro_rules! empty_cfg_data {
        () => {
            CfgData {
                blocks: all_blocks,
                edges: all_edges,
                defs: all_defs,
                uses: all_uses,
                effects: all_effects,
            }
        };
    }

    // Only proceed if the language has a CFG query.
    let loader = normalize_languages::parsers::grammar_loader();
    let cfg_query_src = match loader.get_cfg(grammar_name) {
        Some(q) => q,
        None => return empty_cfg_data!(),
    };
    let ts_language = match loader.get(grammar_name) {
        Ok(l) => l,
        Err(_) => return empty_cfg_data!(),
    };
    let tags_query_src = match loader.get_tags(grammar_name) {
        Some(q) => q,
        None => return empty_cfg_data!(),
    };

    // Parse the file.
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_language).is_err() {
        return empty_cfg_data!();
    }
    let tree = match parser.parse(source_bytes, None) {
        Some(t) => t,
        None => return empty_cfg_data!(),
    };

    // Build a set of (name, start_line) for function/method symbols.
    let func_symbols: Vec<(&FlatSymbol, u32)> = symbols
        .iter()
        .filter_map(|s| {
            let kind = s.kind.as_str();
            if kind == "function" || kind == "method" {
                Some((s, s.start_line as u32))
            } else {
                None
            }
        })
        .collect();

    if func_symbols.is_empty() {
        return empty_cfg_data!();
    }

    // Find function body byte ranges using the tags query.
    let tags_query = match tree_sitter::Query::new(&ts_language, &tags_query_src) {
        Ok(q) => q,
        Err(_) => return empty_cfg_data!(),
    };
    let capture_names = tags_query.capture_names().to_vec();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches_iter = cursor.matches(&tags_query, tree.root_node(), source_bytes);

    // Collect (func_name, def_start, def_end, start_line).
    struct FuncCandidate {
        name: String,
        start_byte: usize,
        end_byte: usize,
        start_line: u32,
    }
    let mut candidates: Vec<FuncCandidate> = Vec::new();
    use streaming_iterator::StreamingIterator as _;
    while let Some(mat) = matches_iter.next() {
        for cap in mat.captures {
            let cap_name = capture_names[cap.index as usize];
            if cap_name.starts_with("name.definition.function")
                || cap_name.starts_with("name.definition.method")
                || cap_name == "name.definition"
            {
                let func_name = cap
                    .node
                    .utf8_text(source_bytes)
                    .unwrap_or("<unknown>")
                    .to_string();
                let def_node = cap.node.parent().unwrap_or(cap.node);
                candidates.push(FuncCandidate {
                    name: func_name,
                    start_byte: def_node.start_byte(),
                    end_byte: def_node.end_byte(),
                    start_line: def_node.start_position().row as u32 + 1,
                });
            }
        }
    }
    drop(matches_iter);

    // For each function symbol, find matching candidate by name + start_line proximity.
    for (sym, sym_start_line) in &func_symbols {
        // Find the candidate whose name matches and start_line is close.
        let candidate = candidates
            .iter()
            .filter(|c| c.name == sym.name)
            .min_by_key(|c| (*sym_start_line as i64 - c.start_line as i64).unsigned_abs());
        let candidate = match candidate {
            Some(c) => c,
            None => continue,
        };

        let body_range = candidate.start_byte..candidate.end_byte;
        let function_id = normalize_cfg::FunctionId {
            file: full_path.to_string_lossy().into_owned(),
            qualified_name: sym.name.clone(),
            start_line: candidate.start_line,
        };

        let cfg = match normalize_cfg::builder::build(
            &tree,
            &cfg_query_src,
            source_bytes,
            function_id,
            body_range,
        ) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let qname = &sym.name;
        let fsl = candidate.start_line;

        for blk in &cfg.blocks {
            all_blocks.push(CfgBlockRow {
                function_qname: qname.clone(),
                function_start_line: fsl,
                block_id: blk.id.0,
                kind: format!("{:?}", blk.kind).to_lowercase(),
                byte_start: blk.byte_range.start,
                byte_end: blk.byte_range.end,
                start_line: blk.start_line,
                end_line: blk.end_line,
            });
            for def in &blk.defs {
                all_defs.push(CfgDefRow {
                    function_qname: qname.clone(),
                    function_start_line: fsl,
                    block_id: blk.id.0,
                    name: def.name.clone(),
                    byte_offset: def.byte_offset,
                    line: def.line,
                });
            }
            for use_ in &blk.uses {
                all_uses.push(CfgUseRow {
                    function_qname: qname.clone(),
                    function_start_line: fsl,
                    block_id: blk.id.0,
                    name: use_.name.clone(),
                    byte_offset: use_.byte_offset,
                    line: use_.line,
                });
            }
            for eff in &blk.effects {
                all_effects.push(CfgEffectRow {
                    function_qname: qname.clone(),
                    function_start_line: fsl,
                    block_id: blk.id.0,
                    kind: format!("{:?}", eff.kind).to_lowercase(),
                    byte_offset: eff.byte_offset,
                    line: eff.line,
                    label: eff.label.clone(),
                });
            }
        }
        for edge in &cfg.edges {
            all_edges.push(CfgEdgeRow {
                function_qname: qname.clone(),
                function_start_line: fsl,
                from_block: edge.from.0,
                to_block: edge.to.0,
                kind: format!("{:?}", edge.kind).to_lowercase(),
                exception_type: edge.exception_type.clone(),
            });
        }
    }

    CfgData {
        blocks: all_blocks,
        edges: all_edges,
        defs: all_defs,
        uses: all_uses,
        effects: all_effects,
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
        partners.sort_unstable_by_key(|p| std::cmp::Reverse(p.1));
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

    // =====================================================================
    // Per-file diagnostics storage tests
    // =====================================================================

    /// Build a FileIndex on an empty tempdir for diagnostics-table tests.
    async fn empty_index(dir: &std::path::Path) -> FileIndex {
        FileIndex::open(&dir.join("index.sqlite"), dir)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn per_file_save_upsert_and_delete_roundtrip() {
        let dir = tempdir().unwrap();
        let index = empty_index(dir.path()).await;

        let upserts = vec![
            ("a.rs".to_string(), vec![1u8, 2, 3]),
            ("b.rs".to_string(), vec![4, 5, 6]),
        ];
        index
            .save_diagnostics_per_file(&upserts, &[], "h1")
            .await
            .unwrap();

        let a = index.load_diagnostics_for_file("a.rs", "h1").await.unwrap();
        let b = index.load_diagnostics_for_file("b.rs", "h1").await.unwrap();
        assert_eq!(a, Some(vec![1, 2, 3]));
        assert_eq!(b, Some(vec![4, 5, 6]));

        // Now delete a.rs and update b.rs in the same call.
        let upserts2 = vec![("b.rs".to_string(), vec![9, 9])];
        let deletes2 = vec!["a.rs".to_string()];
        index
            .save_diagnostics_per_file(&upserts2, &deletes2, "h1")
            .await
            .unwrap();

        assert_eq!(
            index.load_diagnostics_for_file("a.rs", "h1").await.unwrap(),
            None
        );
        assert_eq!(
            index.load_diagnostics_for_file("b.rs", "h1").await.unwrap(),
            Some(vec![9, 9])
        );
    }

    #[tokio::test]
    async fn per_file_save_empty_inputs_is_noop() {
        let dir = tempdir().unwrap();
        let index = empty_index(dir.path()).await;
        // No-op call should succeed and leave the table empty.
        index
            .save_diagnostics_per_file(&[], &[], "h")
            .await
            .unwrap();
        assert!(index.list_diagnostic_paths().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn load_diagnostics_for_file_missing_returns_none() {
        let dir = tempdir().unwrap();
        let index = empty_index(dir.path()).await;
        assert_eq!(
            index
                .load_diagnostics_for_file("nope.rs", "h")
                .await
                .unwrap(),
            None
        );
    }

    /// A row written under one config_hash must be invisible to a load that
    /// presents a different hash — this is what makes the cache safe across
    /// daemon restarts after a config edit.
    #[tokio::test]
    async fn per_file_config_hash_mismatch_is_cache_miss() {
        let dir = tempdir().unwrap();
        let index = empty_index(dir.path()).await;
        index
            .save_diagnostics_per_file(&[("a.rs".to_string(), vec![1])], &[], "old")
            .await
            .unwrap();
        // Same hash → hit.
        assert_eq!(
            index
                .load_diagnostics_for_file("a.rs", "old")
                .await
                .unwrap(),
            Some(vec![1])
        );
        // Different hash → miss.
        assert_eq!(
            index
                .load_diagnostics_for_file("a.rs", "new")
                .await
                .unwrap(),
            None
        );
        let multi = index
            .load_diagnostics_for_files(&["a.rs".to_string()], "new")
            .await
            .unwrap();
        assert!(multi.is_empty());
    }

    /// Same invariant for the per-engine `daemon_diagnostics` table.
    #[tokio::test]
    async fn engine_blob_config_hash_mismatch_is_cache_miss() {
        let dir = tempdir().unwrap();
        let index = empty_index(dir.path()).await;
        index
            .save_diagnostics_blob("syntax", &[7, 8, 9], "old")
            .await
            .unwrap();
        assert_eq!(
            index.load_diagnostics_blob("syntax", "old").await.unwrap(),
            Some(vec![7, 8, 9])
        );
        assert_eq!(
            index.load_diagnostics_blob("syntax", "new").await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn load_diagnostics_for_files_skips_missing() {
        let dir = tempdir().unwrap();
        let index = empty_index(dir.path()).await;
        let upserts = vec![("a.rs".to_string(), vec![1]), ("c.rs".to_string(), vec![3])];
        index
            .save_diagnostics_per_file(&upserts, &[], "h1")
            .await
            .unwrap();

        // Mix present + missing, in a non-canonical order.
        let query = vec![
            "c.rs".to_string(),
            "missing.rs".to_string(),
            "a.rs".to_string(),
        ];
        let mut got: Vec<(String, Vec<u8>)> = index
            .load_diagnostics_for_files(&query, "h1")
            .await
            .unwrap();
        got.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(
            got,
            vec![("a.rs".to_string(), vec![1]), ("c.rs".to_string(), vec![3]),]
        );
    }

    #[tokio::test]
    async fn list_diagnostic_paths_returns_all() {
        let dir = tempdir().unwrap();
        let index = empty_index(dir.path()).await;
        let upserts = vec![
            ("x".to_string(), vec![0]),
            ("y".to_string(), vec![0]),
            ("z".to_string(), vec![0]),
        ];
        index
            .save_diagnostics_per_file(&upserts, &[], "h")
            .await
            .unwrap();
        let mut paths = index.list_diagnostic_paths().await.unwrap();
        paths.sort();
        assert_eq!(paths, vec!["x", "y", "z"]);
    }

    /// Smoke test: a fresh open creates the per-file diagnostics table with the
    /// BLOB column type required by `save_diagnostics_per_file`. (A row inserted
    /// with the wrong column type by an older schema version would fail this
    /// roundtrip — the schema_version != SCHEMA_VERSION migration block at
    /// `FileIndex::open` is responsible for `DROP TABLE IF EXISTS
    /// daemon_diagnostics_per_file` so the new shape is created cleanly.)
    #[tokio::test]
    async fn fresh_open_per_file_table_accepts_blob_roundtrip() {
        let dir = tempdir().unwrap();
        let index = FileIndex::open(&dir.path().join("index.sqlite"), dir.path())
            .await
            .unwrap();
        // The CREATE statement at FileIndex::open declares issues_blob BLOB NOT NULL.
        // Confirm the column type via PRAGMA table_info.
        let mut rows = index
            .conn
            .query("PRAGMA table_info(daemon_diagnostics_per_file)", ())
            .await
            .unwrap();
        let mut col_types: Vec<(String, String)> = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            let name: String = row.get(1).unwrap();
            let ty: String = row.get(2).unwrap();
            col_types.push((name, ty));
        }
        let blob_col = col_types
            .iter()
            .find(|(n, _)| n == "issues_blob")
            .expect("issues_blob column missing");
        assert_eq!(
            blob_col.1.to_uppercase(),
            "BLOB",
            "issues_blob must be BLOB, got {:?}",
            blob_col.1
        );

        // And the BLOB roundtrip itself works.
        index
            .save_diagnostics_per_file(&[("a".to_string(), vec![1, 2, 3])], &[], "h")
            .await
            .unwrap();
        assert_eq!(
            index.load_diagnostics_for_file("a", "h").await.unwrap(),
            Some(vec![1, 2, 3])
        );
    }

    #[tokio::test]
    async fn invalidate_last_indexed_resets_needs_refresh_gate() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let mut index = FileIndex::open(&dir.path().join("index.sqlite"), dir.path())
            .await
            .unwrap();
        index.refresh().await.unwrap();
        // Just-after-refresh, the 60-second gate suppresses needs_refresh.
        assert!(!index.needs_refresh().await);
        index.invalidate_last_indexed().await.unwrap();
        assert!(index.needs_refresh().await);
    }
}
