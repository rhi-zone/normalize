//! Global daemon for watching multiple codebases and keeping indexes fresh.
//!
//! The daemon watches file changes across registered roots and incrementally
//! refreshes their indexes. Index queries go directly to SQLite files.
//!
//! The daemon uses Unix domain sockets for IPC and is only supported on Unix
//! platforms. On Windows all client functions return an unsupported error.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Detect whether a path is a linked git worktree (as opposed to the main repo).
///
/// In a main repo, `.git` is a directory. In a linked worktree (created via
/// `git worktree add`), `.git` is a regular file containing a `gitdir:` pointer
/// to the worktree's state inside the main repo's `.git/worktrees/<name>/`.
///
/// Returns `false` for non-git paths and for main repos. Returns `true` only
/// when `.git` exists and is a file.
pub fn is_git_worktree(root: &Path) -> bool {
    let dot_git = root.join(".git");
    match std::fs::symlink_metadata(&dot_git) {
        Ok(md) => md.file_type().is_file(),
        Err(_) => false,
    }
}

/// An event broadcast to all subscribers when files change or the index refreshes.
///
/// Path fields use `String` (rather than `PathBuf`) so the enum can be rkyv-serialized
/// for binary subscribe frames. The wire format does not need typed paths.
#[derive(
    Debug, Clone, Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
#[rkyv(derive(Debug))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A file was modified, created, or deleted.
    FileChanged {
        /// Canonical path of the file that changed.
        path: String,
        /// The watched root that owns this file.
        root: String,
    },
    /// The index was refreshed after file changes.
    IndexRefreshed {
        /// The root whose index was refreshed.
        root: String,
        /// Number of files reindexed.
        files: u64,
    },
    /// Per-file diagnostic deltas from the most recent prime/refresh.
    ///
    /// One event is broadcast per refresh and contains only the files whose
    /// issues actually changed since the last refresh. Subscribers can apply
    /// these directly without re-pulling the full diagnostic set.
    DiagnosticsUpdated {
        /// Watched root these updates belong to.
        root: String,
        /// Per-file deltas. Each entry is `(relative_path, issues_for_that_file)`.
        /// An empty `issues` Vec means the file is now clean (was previously
        /// dirty, now no issues). Files not in this list have unchanged
        /// diagnostics (or never had any).
        updates: Vec<(String, Vec<normalize_output::diagnostics::Issue>)>,
    },
}

/// Daemon configuration.
#[derive(
    Debug, Clone, Deserialize, serde::Serialize, Default, schemars::JsonSchema, server_less::Config,
)]
#[serde(default)]
pub struct DaemonConfig {
    /// Whether to use the daemon. Default: true
    pub enabled: Option<bool>,
    /// Whether to auto-start the daemon. Default: true
    pub auto_start: Option<bool>,
}

impl DaemonConfig {
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn auto_start(&self) -> bool {
        self.auto_start.unwrap_or(true)
    }
}

/// Daemon request - minimal protocol for managing watched roots.
/// Index queries go directly to SQLite files, not through daemon.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum Request {
    /// Add a root to watch for file changes
    #[serde(rename = "add")]
    Add { root: PathBuf },
    /// Remove a root from watching
    #[serde(rename = "remove")]
    Remove { root: PathBuf },
    /// List all watched roots
    #[serde(rename = "list")]
    List,
    /// Get daemon status
    #[serde(rename = "status")]
    Status,
    /// Shutdown daemon
    #[serde(rename = "shutdown")]
    Shutdown,
    /// Subscribe to file-change and index-refresh events.
    /// The connection stays open; the daemon streams Event JSON lines until
    /// the client disconnects. If `root` is given and not yet watched, it is
    /// added automatically.
    #[serde(rename = "subscribe")]
    Subscribe { root: Option<PathBuf> },
    /// Run rules for a root and return cached diagnostics.
    /// Uses the daemon's diagnostics cache (syntax, fact, native) which is primed
    /// eagerly on every file-change event, giving sub-millisecond results vs.
    /// seconds of cold evaluation.
    #[serde(rename = "run_rules")]
    RunRules {
        root: PathBuf,
        /// Optional rule ID filter (None = all enabled rules).
        filter_ids: Option<Vec<String>>,
        /// Optional single-rule filter (by rule ID); narrows further than filter_ids.
        filter_rule: Option<String>,
        /// Which engine(s) to return results for: "syntax", "fact", "native", or None for all.
        engine: Option<String>,
        /// Filter results to specific files (relative paths). When provided
        /// without other filters/engine, the daemon serves directly from the
        /// per-file diagnostics table.
        #[serde(default)]
        filter_files: Option<Vec<String>>,
    },
    /// Query the daemon's in-memory context index for matching blocks.
    /// The daemon pre-indexes all `.normalize/context/` files and updates them
    /// incrementally via the file watcher, giving near-zero latency responses.
    #[serde(rename = "query_context")]
    QueryContext {
        /// Root directory whose context hierarchy to query.
        root: PathBuf,
        /// Caller-provided key=value pairs for matching (dot-path keys).
        /// Empty means no filtering (equivalent to `--all`).
        #[serde(default)]
        match_keys: Vec<(String, String)>,
        /// When true, return all blocks without filtering.
        #[serde(default)]
        all: bool,
        /// Context directory name inside `.normalize/` (default: "context").
        #[serde(default)]
        dir_name: Option<String>,
    },
    /// Notify the daemon that specific files have changed on disk and should be
    /// re-indexed. This is a direct "push" path that avoids waiting for inotify —
    /// useful after in-process writes (e.g. refactoring edits). Non-fatal if the
    /// root is not currently watched.
    #[serde(rename = "files_changed")]
    FilesChanged {
        /// The watched root the files belong to.
        root: PathBuf,
        /// Absolute or relative paths to the files that changed.
        paths: Vec<PathBuf>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    fn ok(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }
    fn err(msg: &str) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.to_string()),
        }
    }
}

// ============================================================================
// Unix implementation
// ============================================================================

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use crate::config::NormalizeConfig;
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::collections::{HashMap, HashSet};
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc::{Sender, channel};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    use tokio::net::UnixListener;
    use tokio::sync::broadcast;

    /// Hash of the inputs that produced cached diagnostic blobs, used to
    /// invalidate blobs across daemon restarts when the on-disk config has
    /// changed since they were written.
    ///
    /// Inputs (in order):
    /// 1. `CARGO_PKG_VERSION` of the daemon binary — a normalize upgrade that
    ///    changes rule semantics invalidates old blobs.
    /// 2. `.normalize/config.toml` raw bytes (if present).
    /// 3. Every regular file under `.normalize/rules/` (sorted by path),
    ///    file name + bytes.
    ///
    /// This intentionally excludes source files and the index — those have
    /// their own per-file mtime tracking inside the rules engine. Hashing them
    /// here would defeat the cache on every source edit.
    ///
    /// blake3 is used over SHA-2 because it's already a workspace dep
    /// (rkyv-related work pulled it in) and is faster on the small inputs we
    /// hash here.
    pub(super) fn compute_config_hash(root: &Path) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
        if let Ok(content) = std::fs::read(root.join(".normalize/config.toml")) {
            hasher.update(&content);
        }
        if let Ok(entries) = std::fs::read_dir(root.join(".normalize/rules")) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file())
                .collect();
            paths.sort();
            for p in paths {
                if let Ok(content) = std::fs::read(&p) {
                    if let Some(name) = p.file_name() {
                        hasher.update(name.as_encoded_bytes());
                    }
                    hasher.update(&content);
                }
            }
        }
        hasher.finalize().to_hex().to_string()
    }

    /// Hash each regular `.scm` file under `.normalize/rules/` and return
    /// a map from absolute path → blake3 digest.
    ///
    /// Used by [`DaemonServer::reload_config_and_reprime`] to detect which
    /// custom rule files changed since the last prime, so their rule IDs can
    /// be routed through [`DaemonServer::surgical_rerun_rules`] (Tier 2)
    /// instead of falling back to a full reprime (Tier 3).
    fn hash_scm_files(root: &Path) -> HashMap<PathBuf, [u8; 32]> {
        let mut map = HashMap::new();
        let rules_dir = root.join(".normalize/rules");
        let Ok(entries) = std::fs::read_dir(&rules_dir) else {
            return map;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().map(|x| x == "scm").unwrap_or(false))
            .collect();
        paths.sort();
        for p in paths {
            if let Ok(content) = std::fs::read(&p) {
                let digest = blake3::hash(&content);
                map.insert(p, *digest.as_bytes());
            }
        }
        map
    }

    /// Map a `.scm` file path to its rule ID.
    ///
    /// The rule ID is the file stem (e.g. `foo.scm` → `"foo"`). This matches
    /// the convention in [`normalize_syntax_rules::loader::parse_rule_file`]
    /// where the default ID is the file stem and frontmatter can override it.
    /// We use the stem here because reading frontmatter from every changed
    /// file on every config reload would be wasteful; the stem convention
    /// covers the vast majority of custom rules.
    fn scm_path_to_rule_id(path: &Path) -> Option<String> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }

    /// Resolve the directory used for daemon lock + socket files.
    ///
    /// In production this is `~/.config/normalize`. Tests can set
    /// `NORMALIZE_DAEMON_CONFIG_DIR` to use an isolated directory so multiple
    /// daemons can coexist without contending on the user's running daemon.
    fn daemon_config_dir() -> PathBuf {
        if let Some(p) = std::env::var_os("NORMALIZE_DAEMON_CONFIG_DIR") {
            return PathBuf::from(p);
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("normalize")
    }

    /// Get the daemon lock file path (~/.config/normalize/daemon.lock)
    /// Used by the daemon process to ensure only one instance runs.
    fn daemon_lock_path() -> PathBuf {
        daemon_config_dir().join("daemon.lock")
    }

    /// Path to the daemon's log file (`~/.config/normalize/daemon.log`).
    ///
    /// Auto-started daemons run with stdout/stderr connected to `/dev/null`
    /// (see [`DaemonClient::start_daemon`]), so without this sink all `tracing`
    /// output — including the spin-loop WARNs — would be silently discarded.
    /// The path follows the same `daemon_config_dir()` convention as the socket
    /// and lock files, so `NORMALIZE_DAEMON_CONFIG_DIR` redirects it for tests.
    pub fn daemon_log_path() -> PathBuf {
        daemon_config_dir().join("daemon.log")
    }

    /// Environment variable the parent sets on the spawned daemon process so the
    /// child knows to route `tracing` output to [`daemon_log_path`] instead of the
    /// (nulled) inherited stderr. Foreground `daemon run` does not set this, so it
    /// keeps logging to the terminal.
    pub const DAEMON_LOG_ENV: &str = "NORMALIZE_DAEMON_LOG";

    /// If this process is an auto-started daemon (the [`DAEMON_LOG_ENV`] marker is
    /// set), open the daemon log file in append mode for use as the `tracing`
    /// writer. Returns `None` for foreground runs or if the file cannot be opened
    /// (in which case the caller falls back to stderr).
    pub fn open_daemon_log_writer() -> Option<std::fs::File> {
        if std::env::var_os(DAEMON_LOG_ENV).is_none() {
            return None;
        }
        let path = daemon_log_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()
    }

    /// Get the spawn lock file path (~/.config/normalize/daemon-spawn.lock)
    /// Used by clients to serialize daemon spawn attempts.
    fn spawn_lock_path() -> PathBuf {
        daemon_config_dir().join("daemon-spawn.lock")
    }

    /// Try to acquire an exclusive non-blocking flock on the given path.
    /// Returns the locked File on success (caller must keep it alive to hold the lock).
    pub(super) fn try_flock(path: &Path) -> Result<std::fs::File, String> {
        use std::os::unix::io::AsRawFd;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|e| format!("Failed to open lock file: {}", e))?;

        let fd = file.as_raw_fd();
        // SAFETY: FFI to libc::flock; `fd` is a valid open file descriptor for the
        // lifetime of this call and flock has no aliasing requirements.
        let ret = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if ret != 0 {
            return Err("Lock already held".to_string());
        }

        Ok(file)
    }

    /// Acquire the daemon singleton lock. Only the daemon process should call this.
    fn acquire_daemon_lock() -> Result<std::fs::File, String> {
        let path = daemon_lock_path();
        let file = try_flock(&path).map_err(|_| "Another daemon is already running".to_string())?;

        // Write PID for diagnostics
        use std::io::Write as _;
        let mut f = &file;
        let _ = f.write_all(format!("{}\n", std::process::id()).as_bytes());

        Ok(file)
    }

    /// Get global daemon socket path (~/.config/normalize/daemon.sock).
    ///
    /// Tests can override the parent directory by setting
    /// `NORMALIZE_DAEMON_CONFIG_DIR`.
    pub fn global_socket_path() -> PathBuf {
        daemon_config_dir().join("daemon.sock")
    }

    /// Register every directory under `root` (respecting `[walk] exclude` and
    /// `.gitignore`) with `watcher` in [`RecursiveMode::NonRecursive`] mode.
    ///
    /// Returns the number of directories successfully watched.
    ///
    /// Errors on individual directories are logged at warn level and otherwise
    /// ignored — losing one watch is far better than aborting the whole prime
    /// because (e.g.) one subdir hit `EACCES`.
    pub(super) fn watch_tree_non_recursive(
        watcher: &mut RecommendedWatcher,
        root: &Path,
        walk_config: &normalize_rules_config::WalkConfig,
    ) -> usize {
        let mut count = 0usize;
        for entry in normalize_native_rules::walk::gitignore_walk(root, walk_config) {
            if !entry.file_type().is_some_and(|ft| ft.is_dir()) {
                continue;
            }
            match watcher.watch(entry.path(), RecursiveMode::NonRecursive) {
                Ok(()) => count += 1,
                Err(e) => {
                    tracing::warn!(
                        path = %entry.path().display(),
                        error = %e,
                        "failed to add directory to file watcher",
                    );
                }
            }
        }
        count
    }

    /// In-memory cache of parsed context blocks for a watched root.
    ///
    /// Holds pre-parsed blocks keyed by absolute file path. Rebuilt incrementally
    /// when `.normalize/context/` files change via the file watcher, so
    /// `QueryContext` requests return results without any filesystem I/O.
    struct ContextIndex {
        /// Blocks keyed by absolute file path. Each value is a vec of
        /// `(frontmatter_json, body)` pairs for the blocks in that file.
        entries: HashMap<PathBuf, Vec<(serde_json::Value, String)>>,
    }

    impl ContextIndex {
        fn new() -> Self {
            Self {
                entries: HashMap::new(),
            }
        }

        /// (Re)load all `.md` files from `context_dir` into the index.
        fn load_dir(&mut self, context_dir: &Path) {
            let Ok(read) = std::fs::read_dir(context_dir) else {
                return;
            };
            for entry in read.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    self.load_file(&path);
                }
            }
        }

        /// (Re)load a single file into the index, replacing any previous entry.
        fn load_file(&mut self, path: &Path) {
            let Ok(content) = std::fs::read_to_string(path) else {
                self.entries.remove(path);
                return;
            };
            let blocks = normalize_context::parse_blocks(&content);
            let parsed: Vec<(serde_json::Value, String)> = blocks
                .into_iter()
                .map(|b| {
                    let meta = b
                        .frontmatter
                        .map(normalize_context::yaml_to_json)
                        .unwrap_or(serde_json::Value::Null);
                    (meta, b.body)
                })
                .collect();
            self.entries.insert(path.to_path_buf(), parsed);
        }

        /// Remove a file from the index (called when file is deleted).
        fn remove_file(&mut self, path: &Path) {
            self.entries.remove(path);
        }

        /// Query the index for blocks matching `caller_ctx`. Returns blocks in
        /// file-path order (deterministic).
        fn query(
            &self,
            caller_ctx: &normalize_context::CallerContext,
            all_flag: bool,
        ) -> Vec<(PathBuf, serde_json::Value, String)> {
            let mut file_paths: Vec<&PathBuf> = self.entries.keys().collect();
            file_paths.sort();

            let mut results = Vec::new();
            for path in file_paths {
                let blocks = &self.entries[path];
                for (meta, body) in blocks {
                    let yaml_meta = context_json_to_yaml(meta);
                    let parsed_block = normalize_context::ParsedBlock {
                        frontmatter: if yaml_meta.is_null() {
                            None
                        } else {
                            Some(yaml_meta)
                        },
                        body: body.clone(),
                    };
                    if normalize_context::block_matches(&parsed_block, caller_ctx, all_flag) {
                        let trimmed = body.trim().to_string();
                        if !trimmed.is_empty() || parsed_block.frontmatter.is_some() {
                            results.push((path.clone(), meta.clone(), trimmed));
                        }
                    }
                }
            }
            results
        }
    }

    /// Convert a `serde_json::Value` back to `serde_yaml::Value` for block_matches.
    fn context_json_to_yaml(v: &serde_json::Value) -> serde_yaml::Value {
        match v {
            serde_json::Value::Null => serde_yaml::Value::Null,
            serde_json::Value::Bool(b) => serde_yaml::Value::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    serde_yaml::Value::Number(serde_yaml::Number::from(i))
                } else if let Some(f) = n.as_f64() {
                    serde_yaml::Value::Number(serde_yaml::Number::from(f))
                } else {
                    serde_yaml::Value::Null
                }
            }
            serde_json::Value::String(s) => serde_yaml::Value::String(s.clone()),
            serde_json::Value::Array(arr) => {
                serde_yaml::Value::Sequence(arr.iter().map(context_json_to_yaml).collect())
            }
            serde_json::Value::Object(obj) => {
                let mut map = serde_yaml::Mapping::new();
                for (k, v) in obj {
                    map.insert(
                        serde_yaml::Value::String(k.clone()),
                        context_json_to_yaml(v),
                    );
                }
                serde_yaml::Value::Mapping(map)
            }
        }
    }

    /// Coalescing queue of roots whose native rules need re-running.
    ///
    /// Replaces the old unbounded `mpsc::channel::<PathBuf>` that backed
    /// `native_refresh_tx`. The native-rules refresh always re-reads the root's
    /// *current* on-disk state, so N queued refreshes of the same root are
    /// redundant — only one current-state refresh is ever needed. The old channel
    /// kept one entry per `.git/index` notify event, so during heavy git churn the
    /// producer enqueued faster (~5/s) than the consumer drained (~0.9/s), building
    /// an unbounded backlog that took ~20h to drain after activity stopped.
    ///
    /// This structure holds a *set* of dirty roots instead of a queue of events.
    /// Marking the same root dirty M times collapses to a single pending refresh,
    /// so the backlog can never exceed the number of distinct watched roots
    /// regardless of churn. Coalescing is exact, not lossy: because each refresh
    /// recomputes from current disk state, dropping the intermediate
    /// re-computations produces the identical final index.
    ///
    /// Roots are independent — each refresh writes only its own repo's
    /// `.normalize/index.sqlite`, with no cross-root ordering dependency — so the
    /// consumer may drain and process them in any order.
    struct DirtyRoots {
        /// `(set of dirty roots, shutdown flag)`. The shutdown flag lets a
        /// graceful stop wake a blocked consumer so its thread can exit.
        state: Mutex<(HashSet<PathBuf>, bool)>,
        signal: Condvar,
    }

    impl DirtyRoots {
        fn new() -> Self {
            Self {
                state: Mutex::new((HashSet::new(), false)),
                signal: Condvar::new(),
            }
        }

        /// Mark a root dirty (idempotent within a drain cycle) and wake the consumer.
        fn mark(&self, root: PathBuf) {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.0.insert(root);
            self.signal.notify_one();
        }

        /// Signal the consumer to stop and wake it so it can exit. The production
        /// daemon tears down via `std::process::exit` (which abandons the consumer
        /// thread), so this graceful-stop path is currently exercised only by the
        /// blocking-contract test — but it keeps `drain_blocking`'s shutdown
        /// semantics (`None` on empty + shutdown) honest and testable.
        #[cfg(test)]
        fn shutdown(&self) {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.1 = true;
            self.signal.notify_all();
        }

        /// Block until at least one root is dirty (or shutdown is requested), then
        /// take and return all currently-dirty roots. Returns `None` once shutdown
        /// has been requested and the set is empty, signalling the consumer to exit.
        fn drain_blocking(&self) -> Option<Vec<PathBuf>> {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if !guard.0.is_empty() {
                    let drained: Vec<PathBuf> = guard.0.drain().collect();
                    return Some(drained);
                }
                if guard.1 {
                    return None;
                }
                guard = self.signal.wait(guard).unwrap_or_else(|e| e.into_inner());
            }
        }

        /// Number of roots currently pending a refresh. Used by the spin detector
        /// to surface coalesced-queue pressure.
        fn pending_len(&self) -> usize {
            self.state.lock().unwrap_or_else(|e| e.into_inner()).0.len()
        }
    }

    /// A watched root. The shared watcher is owned by `DaemonServer`; this struct
    /// tracks per-root metadata only.
    struct WatchedRoot {
        last_refresh: Instant,
        /// Timestamps of recent `refresh_root` invocations, newest last. Used by
        /// the spin detector to measure refresh density. Trimmed to the
        /// [`SPIN_WINDOW`] on every push so it stays O(refreshes-in-window).
        refresh_window: std::collections::VecDeque<Instant>,
        /// When set and in the future, the dispatch loop backs off (skips
        /// sending) refresh requests for this root. Set when a spin loop is
        /// detected; cleared implicitly once the instant passes. Per-root, never
        /// global — one misbehaving root must not stall the others.
        spin_flagged_until: Option<Instant>,
        /// Whether the diagnostics cache has been fully primed (first run completed).
        /// Diagnostics are persisted to SQLite; this flag just tracks prime state in memory
        /// so the daemon knows when to prime lazily on the first `RunRules` request.
        primed: bool,
        /// Whether the .git/index path was registered with the shared watcher.
        /// Tracked so we can unwatch it when the root is removed.
        has_git_index: bool,
        /// Persistent index connection for this root.
        ///
        /// Opened once in `add_root` and reused for all subsequent reads and writes.
        /// Eliminates the per-write open/close cycle and the SQLite "database is locked"
        /// races that occurred when a new connection was opened while the client held
        /// the same file open for reading.
        index: Arc<std::sync::Mutex<crate::index::FileIndex>>,
        /// Snapshot of the rules + walk config that was active when the cache
        /// was last primed. Used by [`DaemonServer::reload_config`] to compute
        /// a [`ConfigDiff`] and route filter-only changes to the cheap
        /// re-filter-at-serve path instead of a full reprime.
        ///
        /// Replaced with the freshly-parsed config at the end of every prime
        /// or successful reload. `Arc` so the serve paths can read it without
        /// holding the `roots` mutex during the (potentially expensive)
        /// per-issue filter loop.
        cached_rules: Arc<normalize_rules_config::RulesConfig>,
        cached_walk: Arc<normalize_rules_config::WalkConfig>,
        /// Per-file blake3 hashes for every `.scm` file under `.normalize/rules/`
        /// at the time of the last prime or Tier 2 reload. Compared against
        /// freshly-computed hashes in [`DaemonServer::reload_config_and_reprime`]
        /// to identify which custom rules changed, so their IDs can be added to
        /// `rules_to_rerun` and routed through [`DaemonServer::surgical_rerun_rules`]
        /// (Tier 2) instead of triggering a full reprime (Tier 3).
        cached_scm_hashes: HashMap<PathBuf, [u8; 32]>,
        /// Set to `true` when a Tier 1 (filter-only) config reload arrived
        /// since the last prime. The serve paths apply
        /// [`DaemonServer::apply_filter_at_serve`] when set so cached blobs
        /// (still reflecting the *previous* config) are filtered to the new
        /// config before being returned.
        ///
        /// Cleared at the end of every prime — a fresh prime persists blobs
        /// under the current config, so no serve-time filtering is needed.
        serve_filter_pending: bool,
        /// In-memory context index for `.normalize/context/` files.
        /// Updated incrementally when context files change via the file watcher.
        context_index: Arc<Mutex<ContextIndex>>,
    }

    /// Sliding window over which refresh density is measured for spin detection.
    const SPIN_WINDOW: Duration = Duration::from_secs(10);
    /// Minimum number of refreshes within [`SPIN_WINDOW`] before a self-write
    /// overlap is treated as a spin loop. 5-in-10s is well above any plausible
    /// human edit cadence (even rapid save-on-keystroke debounces to one refresh
    /// per 500ms here) but trips quickly once the daemon is re-indexing its own
    /// writes in a tight loop. The overlap check is the real signal; the density
    /// gate just avoids flagging a single coincidental self-write.
    const SPIN_DENSITY_THRESHOLD: usize = 5;
    /// How long to back off refreshes for a root after a spin is detected. Long
    /// enough to break the feedback loop and let any in-flight writes settle,
    /// short enough that legitimate work resumes promptly.
    const SPIN_COOLDOWN: Duration = Duration::from_secs(30);
    /// Cap on retained spin warnings; keeps `status()` bounded.
    const MAX_SPIN_WARNINGS: usize = 32;

    /// A recorded spin-loop detection, surfaced via `normalize daemon status`.
    /// This is the channel that actually reaches the user given the daemon's
    /// stdout/stderr are nulled on auto-start.
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub(super) struct SpinWarning {
        /// The watched root that was re-indexing its own state directory.
        pub root: String,
        /// The overlapping paths inside the root's own index/state dir that
        /// triggered detection (capped to a handful for readability).
        pub overlapping_paths: Vec<String>,
        /// Number of refreshes observed in [`SPIN_WINDOW`] when flagged.
        pub refresh_count: usize,
        /// Seconds since the daemon started, at detection time.
        pub at_uptime_secs: u64,
    }

    /// Global daemon server managing multiple roots.
    struct DaemonServer {
        roots: Mutex<HashMap<PathBuf, WatchedRoot>>,
        /// Recent spin-loop detections, newest last, capped to the most recent
        /// [`MAX_SPIN_WARNINGS`]. Surfaced by `status()` so the failure mode is
        /// self-reporting instead of silently burning CPU.
        spin_warnings: Mutex<Vec<SpinWarning>>,
        refresh_tx: Sender<PathBuf>,
        /// Coalescing queue for native-rules-only refresh requests (triggered by
        /// `.git/index` changes). Per-root "latest wins" — see [`DirtyRoots`].
        native_dirty: Arc<DirtyRoots>,
        /// Sender for config-reload requests (triggered by `.normalize/config.toml`
        /// or `.normalize/rules/**` changes). Handled by clearing all cached
        /// diagnostic blobs and triggering a full reprime.
        config_reload_tx: Sender<PathBuf>,
        start_time: Instant,
        /// Broadcast channel for file-change and index-refresh events.
        /// Subscribers call `event_tx.subscribe()` to get a `broadcast::Receiver`.
        event_tx: broadcast::Sender<Event>,
        /// Handle to the tokio runtime, used to run async code from non-tokio threads
        /// (e.g. the refresh handler thread).
        runtime_handle: tokio::runtime::Handle,
        /// Single shared file watcher for all roots. Consolidating onto one watcher
        /// saves ~3 OS threads per root vs the previous per-root pair of watchers.
        watcher: Mutex<RecommendedWatcher>,
    }

    /// Response type for binary rkyv IPC.
    enum RawResponse {
        /// Serialized rkyv payload.
        Frame(Vec<u8>),
        /// Error message (returned as JSON error frame to client).
        Error(String),
    }

    impl DaemonServer {
        fn new(
            refresh_tx: Sender<PathBuf>,
            native_dirty: Arc<DirtyRoots>,
            config_reload_tx: Sender<PathBuf>,
            runtime_handle: tokio::runtime::Handle,
            watcher: RecommendedWatcher,
        ) -> Self {
            // Capacity 1024: if a subscriber falls behind by more than 1024 events
            // it receives a RecvError::Lagged and we log the drop.
            let (event_tx, _) = broadcast::channel(1024);

            Self {
                roots: Mutex::new(HashMap::new()),
                spin_warnings: Mutex::new(Vec::new()),
                refresh_tx,
                native_dirty,
                config_reload_tx,
                start_time: Instant::now(),
                event_tx,
                runtime_handle,
                watcher: Mutex::new(watcher),
            }
        }

        fn add_root(&self, root: PathBuf) -> Response {
            // Reject dead paths up front — avoids accumulating watchers on
            // worktrees/directories that have been deleted. This also covers
            // the case where a client re-sends a stale root after daemon
            // restart (there is no on-disk root persistence today).
            if !root.exists() {
                eprintln!(
                    "Rejecting add_root for non-existent path: {}",
                    root.display()
                );
                return Response::err(&format!("path does not exist: {}", root.display()));
            }

            let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());

            if roots.contains_key(&root) {
                return Response::ok(
                    serde_json::json!({"added": false, "reason": "already watching"}),
                );
            }

            // Check if indexing is enabled for this root
            let config = NormalizeConfig::load(&root);
            if !config.index.enabled() {
                return Response::err("Indexing disabled for this root");
            }

            // Initial index refresh. Reverse-dep graph is no longer held in memory;
            // it is derived from the SQLite imports table on demand during refresh.
            //
            // The opened FileIndex is stored in WatchedRoot and reused for all
            // subsequent reads/writes, eliminating per-write open/close races.
            let index = match tokio::task::block_in_place(|| {
                self.runtime_handle.block_on(crate::index::open(&root))
            }) {
                Ok(mut idx) => {
                    if let Err(e) =
                        tokio::task::block_in_place(|| self.runtime_handle.block_on(idx.refresh()))
                    {
                        return Response::err(&format!("Failed to index: {}", e));
                    }
                    if let Err(e) = tokio::task::block_in_place(|| {
                        self.runtime_handle
                            .block_on(idx.incremental_call_graph_refresh())
                    }) {
                        eprintln!("Warning: call graph refresh failed: {}", e);
                    }
                    Arc::new(std::sync::Mutex::new(idx))
                }
                Err(e) => return Response::err(&format!("Failed to open index: {}", e)),
            };

            // Register the root with the shared watcher.
            //
            // We walk the tree once respecting `[walk] exclude` and `.gitignore`
            // (via [`gitignore_walk`]) and register each directory
            // NonRecursively. Watching the root recursively would descend into
            // `target/`, `node_modules/`, `.git/`, agent worktrees, etc. — on a
            // typical Rust workspace that produces 50k+ inotify watches and
            // sustained CPU when those trees see activity (cargo build,
            // worktree updates).
            //
            // Newly-created directories at runtime are caught by the dispatch
            // loop's Create-event hook (see `notify_rx` consumer below), which
            // re-runs the same walk on the new subtree.
            let git_index_path = root.join(".git").join("index");
            let has_git_index = git_index_path.exists();
            {
                let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
                let watched_count = watch_tree_non_recursive(&mut watcher, &root, &config.walk);
                tracing::debug!(
                    root = %root.display(),
                    dirs = watched_count,
                    "registered directories with file watcher",
                );
                // Also watch .git/index if it exists so `git add` triggers a native-rules refresh.
                if has_git_index
                    && let Err(e) = watcher.watch(&git_index_path, RecursiveMode::NonRecursive)
                {
                    eprintln!("Warning: failed to watch .git/index for {:?}: {}", root, e);
                }
            }

            // Snapshot the current config so the first reload after add_root
            // has something to diff against. The next prime will refresh this
            // snapshot to reflect what was actually used to produce the cache.
            let initial_config = NormalizeConfig::load(&root);
            let initial_scm_hashes = hash_scm_files(&root);

            // Pre-load context index from the default context directory.
            let mut ctx_index = ContextIndex::new();
            let context_dir = root.join(".normalize").join("context");
            if context_dir.is_dir() {
                ctx_index.load_dir(&context_dir);
            }

            roots.insert(
                root.clone(),
                WatchedRoot {
                    last_refresh: Instant::now(),
                    refresh_window: std::collections::VecDeque::new(),
                    spin_flagged_until: None,
                    primed: false,
                    has_git_index,
                    index,
                    cached_rules: Arc::new(initial_config.rules.clone()),
                    cached_walk: Arc::new(initial_config.walk.clone()),
                    cached_scm_hashes: initial_scm_hashes,
                    serve_filter_pending: false,
                    context_index: Arc::new(Mutex::new(ctx_index)),
                },
            );

            Response::ok(serde_json::json!({"added": true, "root": root}))
        }

        /// Drop any watched roots whose path no longer exists on disk.
        /// Called at daemon startup and available for future periodic sweeps.
        /// Logs each dropped root so disappearance is visible in daemon stderr.
        ///
        /// Note: the current daemon holds roots only in memory for the lifetime
        /// of the process, so at startup this is a no-op. It exists so that if
        /// on-disk root persistence is added later, GC wiring is already in
        /// place and callers don't have to remember to add it.
        fn gc_dead_roots(&self) {
            let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            let dead: Vec<PathBuf> = roots.keys().filter(|p| !p.exists()).cloned().collect();
            for path in dead {
                eprintln!(
                    "Dropping watched root (path no longer exists): {}",
                    path.display()
                );
                roots.remove(&path);
            }
        }

        fn remove_root(&self, root: &Path) -> Response {
            let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(watched) = roots.remove(root) {
                let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
                // We watched each subdirectory NonRecursively in `add_root`;
                // walk the tree the same way to unwatch each one. Tolerate
                // errors — entries that have since been deleted on disk are
                // already gone from the kernel's watch list.
                for entry in
                    normalize_native_rules::walk::gitignore_walk(root, &watched.cached_walk)
                {
                    if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                        let _ = watcher.unwatch(entry.path());
                    }
                }
                let _ = watcher.unwatch(root);
                if watched.has_git_index {
                    let git_index = root.join(".git").join("index");
                    let _ = watcher.unwatch(&git_index);
                }
                Response::ok(serde_json::json!({"removed": true}))
            } else {
                Response::ok(serde_json::json!({"removed": false, "reason": "not watching"}))
            }
        }

        fn list_roots(&self) -> Response {
            let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            let list: Vec<&PathBuf> = roots.keys().collect();
            Response::ok(serde_json::json!(list))
        }

        fn status(&self) -> Response {
            let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            let spin_warnings = self
                .spin_warnings
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            Response::ok(serde_json::json!({
                "uptime_secs": self.start_time.elapsed().as_secs(),
                "roots_watched": roots.len(),
                "pid": std::process::id(),
                "spin_warnings": spin_warnings,
            }))
        }

        /// Retrieve the persistent index for a watched root, or `None` if not watched.
        ///
        /// Callers must release the `roots` lock before calling any async code with the
        /// returned `Arc` to avoid holding both the roots lock and the index lock at once.
        fn get_root_index(
            &self,
            root: &Path,
        ) -> Option<Arc<std::sync::Mutex<crate::index::FileIndex>>> {
            let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            roots.get(root).map(|w| w.index.clone())
        }

        fn handle_request(&self, req: Request) -> Response {
            match req {
                Request::Add { root } => self.add_root(root),
                Request::Remove { root } => self.remove_root(&root),
                Request::List => self.list_roots(),
                Request::Status => self.status(),
                Request::Shutdown => Response::ok(serde_json::json!({"message": "shutting down"})),
                // Subscribe is handled directly in the socket loop -- it keeps the
                // connection open and streams events rather than returning a Response.
                Request::Subscribe { .. } => {
                    Response::err("Subscribe must be handled in the socket loop")
                }
                Request::RunRules {
                    root,
                    filter_ids,
                    filter_rule,
                    engine,
                    filter_files,
                } => self.run_rules(root, filter_ids, filter_rule, engine, filter_files),
                Request::QueryContext {
                    root,
                    match_keys,
                    all,
                    dir_name,
                } => self.query_context(root, match_keys, all, dir_name),
                Request::FilesChanged { root, paths } => self.handle_files_changed(root, paths),
            }
        }

        fn handle_files_changed(&self, root: PathBuf, paths: Vec<PathBuf>) -> Response {
            // Check the root is actually watched; if not, return ok (non-fatal).
            let is_watched = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                roots.contains_key(&root)
            };
            if !is_watched {
                tracing::debug!(
                    root = ?root,
                    "FilesChanged: root not watched, ignoring"
                );
                return Response::ok(
                    serde_json::json!({"queued": false, "reason": "root not watched"}),
                );
            }

            // Broadcast FileChanged events so subscribers (e.g. `normalize daemon watch`)
            // see the push-triggered changes the same as inotify-triggered ones.
            for path in &paths {
                let path_str = path.to_string_lossy().into_owned();
                let root_str = root.to_string_lossy().into_owned();
                let _ = self.event_tx.send(Event::FileChanged {
                    path: path_str,
                    root: root_str,
                });
            }

            // Trigger the same incremental refresh cycle that inotify fires.
            let _ = self.refresh_tx.send(root);

            Response::ok(serde_json::json!({"queued": true}))
        }

        fn query_context(
            &self,
            root: PathBuf,
            match_keys: Vec<(String, String)>,
            all: bool,
            dir_name: Option<String>,
        ) -> Response {
            let dir_name = dir_name.as_deref().unwrap_or("context");

            // Collect context_index arcs for this root and any ancestor roots
            // that the daemon happens to be watching.
            // The primary path: query the in-memory index for this root directly.
            let context_arc = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                roots.get(&root).map(|w| w.context_index.clone())
            };

            let Some(context_arc) = context_arc else {
                return Response::err("root not watched");
            };

            let caller_ctx: normalize_context::CallerContext = match_keys.into_iter().collect();

            // If the request uses a non-default dir_name, we can't serve from the
            // pre-indexed default "context" directory — fall through to the
            // filesystem (return an error so the client falls back to v1).
            if dir_name != "context" {
                return Response::err("non-default dir_name; use filesystem fallback");
            }

            let ctx_index = context_arc.lock().unwrap_or_else(|e| e.into_inner());
            let blocks = ctx_index.query(&caller_ctx, all);

            let json_blocks: Vec<serde_json::Value> = blocks
                .into_iter()
                .map(|(path, metadata, body)| {
                    serde_json::json!({
                        "source": path.to_string_lossy(),
                        "metadata": metadata,
                        "body": body,
                    })
                })
                .collect();

            Response::ok(serde_json::json!(json_blocks))
        }

        fn run_rules(
            &self,
            root: PathBuf,
            filter_ids: Option<Vec<String>>,
            filter_rule: Option<String>,
            engine: Option<String>,
            filter_files: Option<Vec<String>>,
        ) -> Response {
            // Compute current config hash up front so prime/load see the same
            // value. If the on-disk config has changed since we last primed,
            // the load below will miss and we'll fall through to a reprime.
            let config_hash = compute_config_hash(&root);

            let needs_prime = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                match roots.get(&root) {
                    Some(w) => !w.primed,
                    None => return Response::err("root not watched"),
                }
            } || !self.cache_matches_current_config(&root, &config_hash);

            // If cache is not yet primed, do a full prime now (lazily on first request).
            if needs_prime {
                self.prime_diagnostics_cache(&root);
            }

            // Load issues from SQLite for requested engines using the persistent index.
            let idx_arc = match self.get_root_index(&root) {
                Some(a) => a,
                None => return Response::err("root not watched"),
            };

            let include_syntax = engine.as_deref().is_none() || engine.as_deref() == Some("syntax");
            let include_fact = engine.as_deref().is_none() || engine.as_deref() == Some("fact");
            let include_native = engine.as_deref().is_none() || engine.as_deref() == Some("native");

            let engines_to_load: Vec<&str> = [
                include_syntax.then_some("syntax"),
                include_fact.then_some("fact"),
                include_native.then_some("native"),
            ]
            .into_iter()
            .flatten()
            .collect();

            let mut issues: Vec<normalize_output::diagnostics::Issue> = Vec::new();
            for eng in engines_to_load {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                match tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_blob(eng, &config_hash))
                }) {
                    Ok(Some(blob)) => {
                        match rkyv::from_bytes::<
                            Vec<normalize_output::diagnostics::Issue>,
                            rkyv::rancor::Error,
                        >(&blob)
                        {
                            Ok(eng_issues) => issues.extend(eng_issues),
                            Err(e) => {
                                tracing::warn!(
                                    engine = eng,
                                    "failed to deserialize diagnostics: {}",
                                    e
                                )
                            }
                        }
                    }
                    Ok(None) => {} // engine not primed yet — skip
                    Err(e) => tracing::warn!(engine = eng, "failed to load diagnostics: {}", e),
                }
            }

            // Apply pending Tier 1 (filter-only) config-reload changes to the
            // cached blob before any user-supplied filters narrow further.
            if let Some((rules_cfg, _walk_cfg, pending)) = self.get_cached_config(&root)
                && pending
            {
                Self::apply_filter_at_serve(&mut issues, &rules_cfg);
            }

            // Apply optional filters.
            let filter_ids_set: Option<HashSet<String>> =
                filter_ids.map(|ids| ids.into_iter().collect());
            if let Some(ref ids) = filter_ids_set {
                issues.retain(|i| ids.contains(&i.rule_id));
            }
            if let Some(ref rule) = filter_rule {
                issues.retain(|i| i.rule_id == rule.as_str());
            }
            if let Some(files) = filter_files.as_ref() {
                let files_set: HashSet<&String> = files.iter().collect();
                issues.retain(|i| files_set.contains(&i.file));
            }

            match serde_json::to_value(&issues) {
                Ok(v) => Response::ok(serde_json::json!({ "issues": v })),
                Err(e) => Response::err(&format!("failed to serialize issues: {}", e)),
            }
        }

        /// Binary rkyv variant of `run_rules`.  Returns a raw `RawResponse` rather
        /// than a JSON `Response` so the socket handler can write a binary frame.
        fn run_rules_raw(
            &self,
            root: PathBuf,
            filter_ids: Option<Vec<String>>,
            filter_rule: Option<String>,
            engine: Option<String>,
            filter_files: Option<Vec<String>>,
        ) -> RawResponse {
            let config_hash = compute_config_hash(&root);
            let needs_prime = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                match roots.get(&root) {
                    Some(w) => !w.primed,
                    None => return RawResponse::Error("root not watched".into()),
                }
            } || !self.cache_matches_current_config(&root, &config_hash);

            if needs_prime {
                self.prime_diagnostics_cache(&root);
            }

            let idx_arc = match self.get_root_index(&root) {
                Some(a) => a,
                None => return RawResponse::Error("root not watched".into()),
            };

            // Snapshot the current config + filter-pending flag. When pending,
            // even the zero-deser fast paths must deserialize/filter/reserialize
            // so cached blobs honor the Tier 1 reload before the client reads
            // them.
            let cfg_snapshot = self.get_cached_config(&root);
            let serve_filter_pending = cfg_snapshot.as_ref().map(|(_, _, p)| *p).unwrap_or(false);

            // Fast path A: no filter and no engine filter → serve "all" blob directly.
            if filter_ids.is_none()
                && filter_rule.is_none()
                && engine.is_none()
                && filter_files.is_none()
            {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                match tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_blob("all", &config_hash))
                }) {
                    Ok(Some(blob)) => {
                        if !serve_filter_pending {
                            return RawResponse::Frame(blob);
                        }
                        // Tier 1 reload pending — re-filter the cached blob.
                        let rules_cfg = cfg_snapshot.as_ref().map(|(r, _, _)| r.clone());
                        let mut issues: Vec<normalize_output::diagnostics::Issue> =
                            match rkyv::from_bytes::<_, rkyv::rancor::Error>(&blob) {
                                Ok(v) => v,
                                Err(e) => {
                                    return RawResponse::Error(format!(
                                        "rkyv from_bytes (filter-at-serve): {e}"
                                    ));
                                }
                            };
                        if let Some(r) = rules_cfg {
                            Self::apply_filter_at_serve(&mut issues, &r);
                        }
                        return match rkyv::to_bytes::<rkyv::rancor::Error>(&issues) {
                            Ok(b) => RawResponse::Frame(b.to_vec()),
                            Err(e) => RawResponse::Error(format!("rkyv to_bytes: {e}")),
                        };
                    }
                    Ok(None) => return RawResponse::Error("not primed".into()),
                    Err(e) => {
                        return RawResponse::Error(format!("failed to load diagnostics: {e}"));
                    }
                }
            }

            // Fast path B: only `filter_files` is set → serve directly from the
            // per-file diagnostics table without touching the "all" blob.
            if let Some(ref files) = filter_files
                && filter_ids.is_none()
                && filter_rule.is_none()
                && engine.is_none()
            {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                let blobs = match tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_for_files(files, &config_hash))
                }) {
                    Ok(v) => v,
                    Err(e) => {
                        return RawResponse::Error(format!("load_diagnostics_for_files: {e}"));
                    }
                };
                let mut all: Vec<normalize_output::diagnostics::Issue> = Vec::new();
                for (_path, blob) in blobs {
                    match rkyv::from_bytes::<
                        Vec<normalize_output::diagnostics::Issue>,
                        rkyv::rancor::Error,
                    >(&blob)
                    {
                        Ok(v) => all.extend(v),
                        Err(e) => tracing::warn!("rkyv from_bytes per-file: {}", e),
                    }
                }
                if serve_filter_pending && let Some((rules_cfg, _, _)) = cfg_snapshot.as_ref() {
                    Self::apply_filter_at_serve(&mut all, rules_cfg);
                }
                tracing::debug!(
                    files = files.len(),
                    issues = all.len(),
                    "served run_rules from per-file table"
                );
                return match rkyv::to_bytes::<rkyv::rancor::Error>(&all) {
                    Ok(b) => RawResponse::Frame(b.to_vec()),
                    Err(e) => RawResponse::Error(format!("rkyv to_bytes: {e}")),
                };
            }

            // Slow path: filter or engine-specific → load appropriate blobs, filter, re-serialize.
            let include_syntax = engine.as_deref().is_none() || engine.as_deref() == Some("syntax");
            let include_fact = engine.as_deref().is_none() || engine.as_deref() == Some("fact");
            let include_native = engine.as_deref().is_none() || engine.as_deref() == Some("native");

            let engines_to_load: Vec<&str> = [
                include_syntax.then_some("syntax"),
                include_fact.then_some("fact"),
                include_native.then_some("native"),
            ]
            .into_iter()
            .flatten()
            .collect();

            let mut issues: Vec<normalize_output::diagnostics::Issue> = Vec::new();
            for eng in engines_to_load {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                match tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_blob(eng, &config_hash))
                }) {
                    Ok(Some(blob)) => {
                        match rkyv::from_bytes::<
                            Vec<normalize_output::diagnostics::Issue>,
                            rkyv::rancor::Error,
                        >(&blob)
                        {
                            Ok(eng_issues) => issues.extend(eng_issues),
                            Err(e) => {
                                tracing::warn!(engine = eng, "failed to deserialize blob: {}", e)
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(engine = eng, "failed to load diagnostics blob: {}", e)
                    }
                }
            }

            // Apply pending Tier 1 reload filter before user-supplied filters
            // so allow/severity/enabled effects honor the latest config even
            // though the cached blob was produced under the previous one.
            if serve_filter_pending && let Some((rules_cfg, _, _)) = cfg_snapshot.as_ref() {
                Self::apply_filter_at_serve(&mut issues, rules_cfg);
            }

            // Apply filters.
            let filter_ids_set: Option<std::collections::HashSet<String>> =
                filter_ids.map(|ids| ids.into_iter().collect());
            if let Some(ref ids) = filter_ids_set {
                issues.retain(|i| ids.contains(&i.rule_id));
            }
            if let Some(ref rule) = filter_rule {
                issues.retain(|i| i.rule_id == rule.as_str());
            }
            if let Some(files) = filter_files.as_ref() {
                let files_set: std::collections::HashSet<&String> = files.iter().collect();
                issues.retain(|i| files_set.contains(&i.file));
            }

            match rkyv::to_bytes::<rkyv::rancor::Error>(&issues) {
                Ok(b) => RawResponse::Frame(b.to_vec()),
                Err(e) => RawResponse::Error(format!("failed to serialize filtered issues: {e}")),
            }
        }

        /// Returns `true` if the daemon's persisted "all" blob was written under
        /// the current config hash — i.e. cached results are still valid for
        /// this config. Used to detect cross-daemon-restart staleness: a daemon
        /// that was stopped, then had its config edited, then restarted will
        /// see `false` here on the first `RunRules` and reprime.
        ///
        /// Returns `false` if the row is missing (un-primed) *or* the row's
        /// stored hash differs from `current_hash`. Errors talking to SQLite
        /// also return `false` (treat as "must reprime") rather than silently
        /// serving an unverified cache.
        fn cache_matches_current_config(&self, root: &Path, current_hash: &str) -> bool {
            let Some(idx_arc) = self.get_root_index(root) else {
                return false;
            };
            let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
            matches!(
                tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_blob("all", current_hash))
                }),
                Ok(Some(_))
            )
        }

        /// Read the cached `(RulesConfig, WalkConfig)` snapshot for a root,
        /// plus whether a Tier 1 reload is pending (i.e. cached blobs need
        /// re-filtering at serve time).
        ///
        /// Returns `None` if the root is not watched.
        fn get_cached_config(
            &self,
            root: &Path,
        ) -> Option<(
            Arc<normalize_rules_config::RulesConfig>,
            Arc<normalize_rules_config::WalkConfig>,
            bool,
        )> {
            let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            roots.get(root).map(|w| {
                (
                    w.cached_rules.clone(),
                    w.cached_walk.clone(),
                    w.serve_filter_pending,
                )
            })
        }

        /// Apply the current rules config to a Vec of cached findings *at
        /// serve time*. Filter-only config changes (severity / allow / `enabled
        /// = false`) are honored without re-running the rule engines: we drop
        /// disabled rules' findings, drop allow-matched paths, and override
        /// severities from per-rule overrides.
        ///
        /// The cached blobs were produced under the previous config; this
        /// function exists so a Tier 1 reload can take effect immediately
        /// without paying for a full reprime.
        fn apply_filter_at_serve(
            issues: &mut Vec<normalize_output::diagnostics::Issue>,
            rules: &normalize_rules_config::RulesConfig,
        ) {
            use normalize_output::diagnostics::Severity as OutSeverity;
            // Pre-compile global-allow patterns once per call.
            let global_allow: Vec<glob::Pattern> = rules
                .global_allow
                .iter()
                .filter_map(|s| glob::Pattern::new(s).ok())
                .collect();
            // Pre-compile per-rule allow patterns and severity overrides.
            let mut per_rule_allow: HashMap<&str, Vec<glob::Pattern>> = HashMap::new();
            for (id, ovr) in &rules.rules {
                if !ovr.allow.is_empty() {
                    per_rule_allow.insert(
                        id.as_str(),
                        ovr.allow
                            .iter()
                            .filter_map(|s| glob::Pattern::new(s).ok())
                            .collect(),
                    );
                }
            }

            issues.retain_mut(|issue| {
                let ovr = rules.rules.get(&issue.rule_id);
                // Drop findings for explicitly disabled rules.
                if let Some(o) = ovr
                    && o.enabled == Some(false)
                {
                    return false;
                }
                // Drop findings whose file matches any global-allow pattern.
                if global_allow.iter().any(|p| p.matches(&issue.file)) {
                    return false;
                }
                // Drop findings whose file matches a per-rule allow pattern.
                if let Some(pats) = per_rule_allow.get(issue.rule_id.as_str())
                    && pats.iter().any(|p| p.matches(&issue.file))
                {
                    return false;
                }
                // Override severity if the new config sets one. We only know
                // how to map the four canonical severity strings; unknown
                // strings leave the existing severity in place rather than
                // silently downgrading to a default.
                if let Some(sev_str) = ovr.and_then(|o| o.severity.as_deref()) {
                    let new_sev = match sev_str.to_ascii_lowercase().as_str() {
                        "error" => Some(OutSeverity::Error),
                        "warning" | "warn" => Some(OutSeverity::Warning),
                        "info" | "note" => Some(OutSeverity::Info),
                        "hint" => Some(OutSeverity::Hint),
                        _ => None,
                    };
                    if let Some(s) = new_sev {
                        issue.severity = s;
                    }
                }
                true
            });
        }

        /// Prime the diagnostics cache for a root with a full evaluation of all engines.
        /// Results are persisted to the SQLite index and immediately dropped from heap.
        /// Called lazily on the first `RunRules` request or after config invalidation.
        fn prime_diagnostics_cache(&self, root: &Path) {
            let config = NormalizeConfig::load(root);
            let config_hash = compute_config_hash(root);

            // --- Fact rules ---
            let fact_issues: Vec<normalize_output::diagnostics::Issue> = {
                let root_owned = root.to_path_buf();
                let rules = config.rules.clone();
                let handle = self.runtime_handle.clone();
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        let diagnostics =
                            handle.block_on(normalize_rules::collect_fact_diagnostics_incremental(
                                &root_owned,
                                &rules,
                                None,
                                None,
                                None,
                            ));
                        diagnostics
                            .iter()
                            .map(normalize_rules::abi_diagnostic_to_issue)
                            .collect()
                    })
                    .expect("failed to spawn fact prime thread")
                    .join()
                    .expect("fact prime thread panicked")
            };

            // --- Syntax rules ---
            let syntax_issues: Vec<normalize_output::diagnostics::Issue> = {
                let root_owned = root.to_path_buf();
                let rules_config = config.rules.clone();
                let walk_config = config.walk.clone();
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        let debug_flags = normalize_syntax_rules::DebugFlags::default();
                        let path_filter = normalize_rules_config::PathFilter::default();
                        let findings = normalize_rules::cmd_rules::run_syntax_rules(
                            &root_owned,
                            &root_owned,
                            None,
                            None,
                            None,
                            &rules_config,
                            &debug_flags,
                            None,
                            &path_filter,
                            &walk_config,
                        );
                        findings
                            .iter()
                            .map(|f| normalize_rules::finding_to_issue(f, &root_owned))
                            .collect()
                    })
                    .expect("failed to spawn syntax prime thread")
                    .join()
                    .expect("syntax prime thread panicked")
            };

            // --- Native rules ---
            let native_issues: Vec<normalize_output::diagnostics::Issue> = {
                let root_owned = root.to_path_buf();
                let rules_config = config.rules.clone();
                let walk_config = config.walk.clone();
                let handle = self.runtime_handle.clone();
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        handle.block_on(Self::run_native_rules(
                            &root_owned,
                            &rules_config,
                            &walk_config,
                        ))
                    })
                    .expect("failed to spawn native prime thread")
                    .join()
                    .expect("native prime thread panicked")
            };

            tracing::info!(
                root = ?root,
                syntax = syntax_issues.len(),
                fact = fact_issues.len(),
                native = native_issues.len(),
                "diagnostics cache fully primed"
            );

            // Persist to SQLite and drop the Vecs immediately.
            self.save_diagnostics_to_index(root, "syntax", &syntax_issues, &config_hash);
            self.save_diagnostics_to_index(root, "fact", &fact_issues, &config_hash);
            self.save_diagnostics_to_index(root, "native", &native_issues, &config_hash);
            // Build "all" combined blob for fast no-filter hot path.
            self.save_all_blob(
                root,
                &syntax_issues,
                &fact_issues,
                &native_issues,
                &config_hash,
            );
            // Per-file table + JSON mirror for ephemeral consumers.
            let delta = self.save_per_file_diagnostics(
                root,
                &syntax_issues,
                &fact_issues,
                &native_issues,
                &config_hash,
            );
            self.write_json_mirror(root, &syntax_issues, &fact_issues, &native_issues);
            drop(syntax_issues);
            drop(fact_issues);
            drop(native_issues);

            self.broadcast_diagnostics_delta(root, delta);

            let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(watched) = roots.get_mut(root) {
                watched.primed = true;
                // Refresh the cached config snapshot so subsequent reload diffs
                // are computed against what actually produced the persisted
                // blobs.
                watched.cached_rules = Arc::new(config.rules.clone());
                watched.cached_walk = Arc::new(config.walk.clone());
                // Refresh the .scm hash snapshot so the next
                // reload_config_and_reprime can diff against what was actually
                // used to produce the blobs.
                watched.cached_scm_hashes = hash_scm_files(root);
                // Persisted blobs now match the cached config — no serve-time
                // filtering needed until the next config reload.
                watched.serve_filter_pending = false;
            }
        }

        /// Broadcast a per-file diagnostic delta to subscribers. SendError
        /// from broadcast = no active subscribers, which is fine.
        fn broadcast_diagnostics_delta(
            &self,
            root: &Path,
            delta: Vec<(String, Vec<normalize_output::diagnostics::Issue>)>,
        ) {
            if delta.is_empty() {
                return;
            }
            let _ = self.event_tx.send(Event::DiagnosticsUpdated {
                root: root.to_string_lossy().into_owned(),
                updates: delta,
            });
        }

        /// Serialize issues to rkyv and write them to the `daemon_diagnostics` table.
        /// Failures are logged as warnings; they do not abort the refresh.
        ///
        /// Uses the persistent index connection stored in `WatchedRoot` to avoid the
        /// per-write open/close cycle that caused "database is locked" races.
        fn save_diagnostics_to_index(
            &self,
            root: &Path,
            engine: &str,
            issues: &[normalize_output::diagnostics::Issue],
            config_hash: &str,
        ) {
            let issues_vec: Vec<normalize_output::diagnostics::Issue> = issues.to_vec();
            let blob = match rkyv::to_bytes::<rkyv::rancor::Error>(&issues_vec) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(engine, "failed to serialize diagnostics to rkyv: {}", e);
                    return;
                }
            };
            let idx_arc = match self.get_root_index(root) {
                Some(a) => a,
                None => {
                    tracing::warn!(engine, "failed to save diagnostics: root not watched");
                    return;
                }
            };
            let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
            if let Err(e) = tokio::task::block_in_place(|| {
                self.runtime_handle
                    .block_on(idx.save_diagnostics_blob(engine, &blob, config_hash))
            }) {
                tracing::warn!(engine, "failed to write diagnostics to SQLite: {}", e);
            }
        }

        /// Build the combined "all" blob from three engine slices and persist it.
        fn save_all_blob(
            &self,
            root: &Path,
            syntax: &[normalize_output::diagnostics::Issue],
            fact: &[normalize_output::diagnostics::Issue],
            native: &[normalize_output::diagnostics::Issue],
            config_hash: &str,
        ) {
            let mut all: Vec<normalize_output::diagnostics::Issue> =
                Vec::with_capacity(syntax.len() + fact.len() + native.len());
            all.extend_from_slice(syntax);
            all.extend_from_slice(fact);
            all.extend_from_slice(native);
            self.save_diagnostics_to_index(root, "all", &all, config_hash);
        }

        /// Reload the three per-engine blobs from SQLite, merge them, and
        /// re-persist the "all" blob.  Used when only one engine is refreshed
        /// (e.g. `refresh_native_rules`) so the "all" blob stays coherent.
        fn rebuild_all_blob(&self, root: &Path) {
            let idx_arc = match self.get_root_index(root) {
                Some(a) => a,
                None => {
                    tracing::warn!("failed to rebuild all blob: root not watched");
                    return;
                }
            };
            let config_hash = compute_config_hash(root);
            let mut all: Vec<normalize_output::diagnostics::Issue> = Vec::new();
            for eng in &["syntax", "fact", "native"] {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                if let Ok(Some(blob)) = tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_blob(eng, &config_hash))
                }) && let Ok(issues) = rkyv::from_bytes::<
                    Vec<normalize_output::diagnostics::Issue>,
                    rkyv::rancor::Error,
                >(&blob)
                {
                    all.extend(issues);
                }
            }
            self.save_diagnostics_to_index(root, "all", &all, &config_hash);
        }

        /// Group all current issues by path and reconcile per-file storage.
        /// Called after every prime or incremental refresh so the per-file
        /// table reflects the same state as the per-engine "all" blob.
        ///
        /// Returns the per-file delta — one entry per path whose diagnostics
        /// actually changed since the last refresh. Each entry is
        /// `(relative_path, issues_for_that_file)`. An empty `issues` Vec
        /// means the file is now clean (had a row, now does not). The caller
        /// broadcasts this delta as `Event::DiagnosticsUpdated`.
        ///
        /// Files whose blob did not change are *not* in the delta — without
        /// this filter, every refresh would emit every file with issues
        /// regardless of whether its issues changed.
        fn save_per_file_diagnostics(
            &self,
            root: &Path,
            syntax: &[normalize_output::diagnostics::Issue],
            fact: &[normalize_output::diagnostics::Issue],
            native: &[normalize_output::diagnostics::Issue],
            config_hash: &str,
        ) -> Vec<(String, Vec<normalize_output::diagnostics::Issue>)> {
            // Group all issues by path.
            let mut by_path: HashMap<String, Vec<normalize_output::diagnostics::Issue>> =
                HashMap::new();
            for issue in syntax.iter().chain(fact.iter()).chain(native.iter()) {
                by_path
                    .entry(issue.file.clone())
                    .or_default()
                    .push(issue.clone());
            }

            let Some(idx_arc) = self.get_root_index(root) else {
                return Vec::new();
            };
            let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());

            // Files currently in the table that no longer have issues -> delete.
            let existing_paths: Vec<String> = match tokio::task::block_in_place(|| {
                self.runtime_handle.block_on(idx.list_diagnostic_paths())
            }) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("failed to list diagnostic paths: {}", e);
                    return Vec::new();
                }
            };
            let new_paths: HashSet<&String> = by_path.keys().collect();
            let deletes: Vec<String> = existing_paths
                .iter()
                .filter(|p| !new_paths.contains(*p))
                .cloned()
                .collect();

            // Load existing blobs for the paths we are about to upsert so we
            // can skip rows whose serialized bytes are identical -- otherwise
            // every refresh would mark every file with issues as "updated".
            let candidate_paths: Vec<String> = by_path.keys().cloned().collect();
            let existing_blobs: HashMap<String, Vec<u8>> = match tokio::task::block_in_place(|| {
                self.runtime_handle
                    .block_on(idx.load_diagnostics_for_files(&candidate_paths, config_hash))
            }) {
                Ok(rows) => rows.into_iter().collect(),
                Err(e) => {
                    tracing::warn!("failed to load existing per-file blobs: {}", e);
                    HashMap::new()
                }
            };

            // Serialize each file's issues to rkyv, comparing against the
            // existing blob to elide unchanged rows from both the write batch
            // and the broadcast delta.
            let mut upserts: Vec<(String, Vec<u8>)> = Vec::with_capacity(by_path.len());
            let mut delta: Vec<(String, Vec<normalize_output::diagnostics::Issue>)> =
                Vec::with_capacity(by_path.len() + deletes.len());
            for (path, issues) in by_path {
                let bytes = match rkyv::to_bytes::<rkyv::rancor::Error>(&issues) {
                    Ok(b) => b.to_vec(),
                    Err(e) => {
                        tracing::warn!("rkyv serialize per-file diagnostics: {}", e);
                        continue;
                    }
                };
                if existing_blobs.get(&path).is_some_and(|prev| prev == &bytes) {
                    // Unchanged -- skip both write and broadcast.
                    continue;
                }
                delta.push((path.clone(), issues));
                upserts.push((path, bytes));
            }
            for path in &deletes {
                delta.push((path.clone(), Vec::new()));
            }

            if let Err(e) = tokio::task::block_in_place(|| {
                self.runtime_handle.block_on(idx.save_diagnostics_per_file(
                    &upserts,
                    &deletes,
                    config_hash,
                ))
            }) {
                tracing::warn!("failed to save per-file diagnostics: {}", e);
            }

            delta
        }

        /// Rebuild the per-file diagnostics table by reading the three per-engine
        /// blobs from SQLite. Used when only one engine has been refreshed
        /// (e.g. `refresh_native_rules`).
        fn rebuild_per_file_diagnostics(&self, root: &Path) {
            let Some(idx_arc) = self.get_root_index(root) else {
                return;
            };
            let config_hash = compute_config_hash(root);
            let mut syntax: Vec<normalize_output::diagnostics::Issue> = Vec::new();
            let mut fact: Vec<normalize_output::diagnostics::Issue> = Vec::new();
            let mut native: Vec<normalize_output::diagnostics::Issue> = Vec::new();
            for (eng, sink) in [
                ("syntax", &mut syntax),
                ("fact", &mut fact),
                ("native", &mut native),
            ] {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                if let Ok(Some(blob)) = tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_blob(eng, &config_hash))
                }) && let Ok(issues) = rkyv::from_bytes::<
                    Vec<normalize_output::diagnostics::Issue>,
                    rkyv::rancor::Error,
                >(&blob)
                {
                    *sink = issues;
                }
            }
            let delta = self.save_per_file_diagnostics(root, &syntax, &fact, &native, &config_hash);
            self.write_json_mirror(root, &syntax, &fact, &native);
            self.broadcast_diagnostics_delta(root, delta);
        }

        /// Write `.normalize/diagnostics.json` atomically.
        /// File is keyed by relative path; value is `Vec<Issue>`. Files with
        /// no issues are omitted. `BTreeMap` ensures deterministic key order.
        fn write_json_mirror(
            &self,
            root: &Path,
            syntax: &[normalize_output::diagnostics::Issue],
            fact: &[normalize_output::diagnostics::Issue],
            native: &[normalize_output::diagnostics::Issue],
        ) {
            use std::collections::BTreeMap;

            let mut by_path: BTreeMap<String, Vec<&normalize_output::diagnostics::Issue>> =
                BTreeMap::new();
            for issue in syntax.iter().chain(fact.iter()).chain(native.iter()) {
                by_path.entry(issue.file.clone()).or_default().push(issue);
            }

            let dir = root.join(".normalize");
            if let Err(e) = std::fs::create_dir_all(&dir) {
                tracing::warn!("failed to create .normalize dir: {}", e);
                return;
            }

            let final_path = dir.join("diagnostics.json");
            let tmp_path = dir.join("diagnostics.json.tmp");

            let json = match serde_json::to_vec_pretty(&by_path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("failed to serialize diagnostics.json: {}", e);
                    return;
                }
            };

            if let Err(e) = std::fs::write(&tmp_path, &json) {
                tracing::warn!("failed to write diagnostics.json.tmp: {}", e);
                return;
            }
            if let Err(e) = std::fs::rename(&tmp_path, &final_path) {
                tracing::warn!("failed to rename diagnostics.json: {}", e);
            }
        }

        /// Run all native rules and return unified Issues.
        async fn run_native_rules(
            root: &Path,
            rules_config: &normalize_rules::RulesConfig,
            walk_config: &normalize_rules_config::WalkConfig,
        ) -> Vec<normalize_output::diagnostics::Issue> {
            use normalize_output::diagnostics::DiagnosticsReport;

            /// Typed config for summary rules (mirrors normalize-rules service.rs).
            #[derive(serde::Deserialize, Default)]
            struct SummaryRuleConfig {
                #[serde(
                    default,
                    deserialize_with = "normalize_rules_config::deserialize_one_or_many"
                )]
                filenames: Vec<String>,
                #[serde(
                    default,
                    deserialize_with = "normalize_rules_config::deserialize_one_or_many"
                )]
                paths: Vec<String>,
            }

            // Read config for summary rules.
            let missing_summary_cfg: SummaryRuleConfig = rules_config
                .rules
                .get("missing-summary")
                .map(|r| r.rule_config())
                .unwrap_or_default();
            let stale_summary_cfg: SummaryRuleConfig = rules_config
                .rules
                .get("stale-summary")
                .map(|r| r.rule_config())
                .unwrap_or_default();
            let threshold = 10;

            let root_owned = root.to_path_buf();

            // Read config for boundary-violations rule.
            let boundary_cfg: normalize_native_rules::BoundaryViolationsConfig = rules_config
                .rules
                .get("boundary-violations")
                .map(|r| r.rule_config())
                .unwrap_or_default();
            let boundaries: Vec<normalize_native_rules::Boundary> = boundary_cfg
                .boundaries
                .iter()
                .filter_map(|s| normalize_native_rules::parse_boundary(s))
                .collect();

            let (
                missing_res,
                summary_res,
                stale_res,
                examples_res,
                refs_res,
                ratchet_res,
                budget_res,
                boundary_res,
            ) = tokio::join!(
                tokio::task::spawn_blocking({
                    let r = root_owned.clone();
                    let fnames = missing_summary_cfg.filenames.clone();
                    let paths = missing_summary_cfg.paths.clone();
                    let wc = walk_config.clone();
                    move || {
                        normalize_native_rules::build_missing_summary_report(
                            &r, threshold, &fnames, &paths, &wc,
                        )
                    }
                }),
                tokio::task::spawn_blocking({
                    let r = root_owned.clone();
                    let fnames = stale_summary_cfg.filenames.clone();
                    let paths = stale_summary_cfg.paths.clone();
                    let wc = walk_config.clone();
                    move || {
                        normalize_native_rules::build_stale_summary_report(
                            &r, threshold, &fnames, &paths, &wc,
                        )
                    }
                }),
                tokio::task::spawn_blocking({
                    let r = root_owned.clone();
                    let wc = walk_config.clone();
                    move || normalize_native_rules::build_stale_docs_report(&r, &wc)
                }),
                tokio::task::spawn_blocking({
                    let r = root_owned.clone();
                    let wc = walk_config.clone();
                    move || normalize_native_rules::build_check_examples_report(&r, &wc)
                }),
                normalize_native_rules::build_check_refs_report(&root_owned, walk_config),
                tokio::task::spawn_blocking({
                    let r = root_owned.clone();
                    move || normalize_native_rules::build_ratchet_report(&r)
                }),
                tokio::task::spawn_blocking({
                    let r = root_owned.clone();
                    move || normalize_native_rules::build_budget_report(&r)
                }),
                normalize_native_rules::build_boundary_violations_report(&root_owned, &boundaries),
            );

            let mut report = DiagnosticsReport::new();
            if let Ok(r) = missing_res {
                report.merge(r.into());
            }
            if let Ok(r) = summary_res {
                report.merge(r.into());
            }
            if let Ok(r) = stale_res {
                report.merge(r.into());
            }
            if let Ok(r) = examples_res {
                report.merge(r.into());
            }
            if let Ok(r) = refs_res {
                report.merge(r.into());
            }
            if let Ok(r) = ratchet_res {
                report.merge(r.into());
            }
            if let Ok(r) = budget_res {
                report.merge(r.into());
            }
            report.merge(boundary_res);

            normalize_rules::apply_native_rules_config(&mut report, rules_config);
            report.issues
        }

        /// Record a refresh and decide whether the root is in a spin loop.
        ///
        /// The signal is *overlap*, not raw frequency: a refresh whose changed
        /// set touches the root's own index/state directory means the daemon is
        /// re-indexing files it wrote itself (index.sqlite, journals,
        /// diagnostics.json) — the exact feedback loop that burned CPU for hours
        /// twice. A human editing source files produces zero changed paths inside
        /// that dir, so a busy repo never trips this. The density gate
        /// ([`SPIN_DENSITY_THRESHOLD`] refreshes in [`SPIN_WINDOW`]) just avoids
        /// flagging a single coincidental self-write (e.g. a one-off config
        /// rewrite).
        ///
        /// `state_dir` is derived from the root's configured index location
        /// ([`crate::paths::get_normalize_dir`]), not a hardcoded `.normalize`.
        /// On detection: sets `spin_flagged_until`, emits a WARN, and records a
        /// [`SpinWarning`] for `status()`. Returns `true` when a spin was flagged.
        ///
        /// Pure enough to unit-test: pass a synthetic changed set + state dir.
        fn record_refresh_and_detect_spin(
            &self,
            root: &Path,
            changed: &[PathBuf],
            now: Instant,
        ) -> bool {
            let state_dir = crate::paths::get_normalize_dir(root);

            // Update the sliding refresh window and measure density.
            let refresh_count = {
                let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                let Some(watched) = roots.get_mut(root) else {
                    return false;
                };
                watched.refresh_window.push_back(now);
                while let Some(front) = watched.refresh_window.front() {
                    if now.duration_since(*front) > SPIN_WINDOW {
                        watched.refresh_window.pop_front();
                    } else {
                        break;
                    }
                }
                watched.refresh_window.len()
            };

            // Overlap check: any changed path under the root's own state dir.
            let overlapping: Vec<String> = changed
                .iter()
                .filter(|p| p.starts_with(&state_dir))
                .take(8)
                .map(|p| p.display().to_string())
                .collect();

            if overlapping.is_empty() || refresh_count < SPIN_DENSITY_THRESHOLD {
                return false;
            }

            // Spin detected: flag backoff, warn loudly, record for status.
            {
                let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(watched) = roots.get_mut(root) {
                    watched.spin_flagged_until = Some(now + SPIN_COOLDOWN);
                }
            }

            tracing::warn!(
                root = %root.display(),
                refreshes_in_window = refresh_count,
                overlapping = ?overlapping,
                "spin loop detected: daemon is re-indexing its own state dir; backing off {}s",
                SPIN_COOLDOWN.as_secs(),
            );

            {
                let mut warnings = self.spin_warnings.lock().unwrap_or_else(|e| e.into_inner());
                warnings.push(SpinWarning {
                    root: root.display().to_string(),
                    overlapping_paths: overlapping,
                    refresh_count,
                    at_uptime_secs: self.start_time.elapsed().as_secs(),
                });
                let len = warnings.len();
                if len > MAX_SPIN_WARNINGS {
                    warnings.drain(0..len - MAX_SPIN_WARNINGS);
                }
            }
            true
        }

        /// Spin detection for the native-rules (`.git/index`-driven) refresh path.
        ///
        /// The original [`record_refresh_and_detect_spin`] only fires when the
        /// changed-file set overlaps the root's own `.normalize/` state dir. A
        /// `.git/index`-driven backlog touches paths *outside* `.normalize/`, so
        /// that detector never trips on it — which is exactly the failure mode that
        /// let a 57k-deep native-refresh backlog peg two cores for hours.
        ///
        /// This is the complementary signal: it records the native-refresh into the
        /// per-root sliding window and flags a spin when the refresh density is high
        /// (≥[`SPIN_DENSITY_THRESHOLD`] in [`SPIN_WINDOW`]) **and** the coalescing
        /// queue still has this or other roots pending. With coalescing in place a
        /// true runaway is nearly impossible — the queue can hold at most one entry
        /// per watched root — but a pathological producer (e.g. a tool rewriting
        /// `.git/index` in a tight loop) can still drive repeated current-state
        /// refreshes; this catches that and backs the root off.
        ///
        /// Unlike the source-file detector, the backoff is consulted by the
        /// dispatch loop's `.git/index` branch (see the dispatch loop) so a flagged
        /// root stops being marked dirty until [`SPIN_COOLDOWN`] elapses.
        fn record_native_refresh_and_detect_spin(&self, root: &Path, now: Instant) -> bool {
            let refresh_count = {
                let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                let Some(watched) = roots.get_mut(root) else {
                    return false;
                };
                watched.refresh_window.push_back(now);
                while let Some(front) = watched.refresh_window.front() {
                    if now.duration_since(*front) > SPIN_WINDOW {
                        watched.refresh_window.pop_front();
                    } else {
                        break;
                    }
                }
                watched.refresh_window.len()
            };

            // Queue pressure: roots still pending after this drain. A healthy
            // system drains to zero between churn bursts; sustained pressure
            // alongside high density indicates a producer outrunning the consumer.
            let pending = self.native_dirty.pending_len();
            if refresh_count < SPIN_DENSITY_THRESHOLD || pending == 0 {
                return false;
            }

            {
                let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(watched) = roots.get_mut(root) {
                    watched.spin_flagged_until = Some(now + SPIN_COOLDOWN);
                }
            }

            tracing::warn!(
                root = %root.display(),
                refreshes_in_window = refresh_count,
                pending_roots = pending,
                "spin loop detected: native-rules refresh is running at high density \
                 with a non-draining queue; backing off {}s",
                SPIN_COOLDOWN.as_secs(),
            );

            {
                let mut warnings = self.spin_warnings.lock().unwrap_or_else(|e| e.into_inner());
                warnings.push(SpinWarning {
                    root: root.display().to_string(),
                    overlapping_paths: vec![format!(".git/index ({pending} roots pending)")],
                    refresh_count,
                    at_uptime_secs: self.start_time.elapsed().as_secs(),
                });
                let len = warnings.len();
                if len > MAX_SPIN_WARNINGS {
                    warnings.drain(0..len - MAX_SPIN_WARNINGS);
                }
            }
            true
        }

        fn refresh_root(&self, root: &Path) {
            // Check root is still watched and retrieve the persistent index.
            let idx_arc = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                match roots.get(root) {
                    Some(w) => w.index.clone(),
                    None => return,
                }
            };

            {
                let mut idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                match self
                    .runtime_handle
                    .block_on(idx.incremental_refresh_force())
                {
                    Ok(changed) if !changed.is_empty() => {
                        // Spin detection: if this refresh is picking up the
                        // daemon's own writes under the root's state dir at high
                        // density, flag a backoff and warn. We still proceed with
                        // this refresh — backoff only suppresses *future* sends
                        // (see the dispatch loop); we never silently drop indexing.
                        self.record_refresh_and_detect_spin(root, &changed, Instant::now());

                        if let Err(e) = self
                            .runtime_handle
                            .block_on(idx.incremental_call_graph_refresh())
                        {
                            eprintln!("Call graph refresh error for {:?}: {}", root, e);
                        }

                        // Compute reverse dependents from SQLite imports table.
                        // This avoids keeping a full HashMap in memory; we query
                        // for just the affected files' reverse deps on each refresh.
                        let affected_vec: Vec<PathBuf> = {
                            let changed_set: HashSet<&PathBuf> = changed.iter().collect();
                            let mut affected: HashSet<PathBuf> = changed.iter().cloned().collect();

                            // Build a transient reverse-dep lookup from current SQLite data.
                            match self
                                .runtime_handle
                                .block_on(idx.all_resolved_import_edges())
                            {
                                Ok(edges) => {
                                    // edges: (importer_rel, imported_rel)
                                    // Build imported_abs -> {importer_abs} map for changed files only.
                                    let mut rev_deps: HashMap<PathBuf, Vec<PathBuf>> =
                                        HashMap::new();
                                    for (importer_rel, imported_rel) in edges {
                                        let imported_abs = root.join(&imported_rel);
                                        if changed_set.contains(&imported_abs) {
                                            let importer_abs = root.join(&importer_rel);
                                            rev_deps
                                                .entry(imported_abs)
                                                .or_default()
                                                .push(importer_abs);
                                        }
                                    }
                                    for changed_file in &changed {
                                        if let Some(dependents) = rev_deps.get(changed_file) {
                                            for dep in dependents {
                                                if !changed_set.contains(dep) {
                                                    affected.insert(dep.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => tracing::warn!(
                                    root = ?root,
                                    "failed to query import edges for rev-dep: {}",
                                    e
                                ),
                            }

                            affected.into_iter().collect()
                        };

                        tracing::info!(
                            changed = changed.len(),
                            affected = affected_vec.len(),
                            root = ?root,
                            "index refreshed"
                        );

                        let config = NormalizeConfig::load(root);
                        let config_hash = compute_config_hash(root);

                        // --- Fact rules (incremental via ENGINE_CACHE) ---
                        let fact_issues: Vec<normalize_output::diagnostics::Issue> = {
                            let diagnostics = self.runtime_handle.block_on(
                                normalize_rules::collect_fact_diagnostics_incremental(
                                    root,
                                    &config.rules,
                                    None,
                                    None,
                                    Some(&affected_vec),
                                ),
                            );
                            diagnostics
                                .iter()
                                .map(normalize_rules::abi_diagnostic_to_issue)
                                .collect()
                        };

                        // --- Syntax rules (re-run on affected files, merge into cache) ---
                        let syntax_issues: Vec<normalize_output::diagnostics::Issue> = {
                            let root_owned = root.to_path_buf();
                            let rules_config = config.rules.clone();
                            let walk_config = config.walk.clone();
                            let affected_for_syntax = affected_vec.clone();
                            let new_syntax_issues: Vec<normalize_output::diagnostics::Issue> =
                                std::thread::Builder::new()
                                    .stack_size(64 * 1024 * 1024)
                                    .spawn(move || {
                                        let debug_flags =
                                            normalize_syntax_rules::DebugFlags::default();
                                        let path_filter =
                                            normalize_rules_config::PathFilter::default();
                                        let findings = normalize_rules::cmd_rules::run_syntax_rules(
                                            &root_owned,
                                            &root_owned,
                                            None,
                                            None,
                                            None,
                                            &rules_config,
                                            &debug_flags,
                                            Some(&affected_for_syntax),
                                            &path_filter,
                                            &walk_config,
                                        );
                                        findings
                                            .iter()
                                            .map(|f| {
                                                normalize_rules::finding_to_issue(f, &root_owned)
                                            })
                                            .collect()
                                    })
                                    .expect("failed to spawn syntax refresh thread")
                                    .join()
                                    .expect("syntax refresh thread panicked");

                            // Merge: load existing syntax issues, remove stale, add new.
                            let affected_rel: HashSet<String> = affected_vec
                                .iter()
                                .filter_map(|p| {
                                    p.strip_prefix(root)
                                        .ok()
                                        .map(|r| r.to_string_lossy().to_string())
                                })
                                .collect();
                            let mut merged: Vec<normalize_output::diagnostics::Issue> = match self
                                .runtime_handle
                                .block_on(idx.load_diagnostics_blob("syntax", &config_hash))
                            {
                                Ok(Some(blob)) => rkyv::from_bytes::<
                                    Vec<normalize_output::diagnostics::Issue>,
                                    rkyv::rancor::Error,
                                >(&blob)
                                .unwrap_or_default(),
                                _ => Vec::new(),
                            };
                            merged.retain(|i| !affected_rel.contains(&i.file));
                            merged.extend(new_syntax_issues);
                            merged
                        };

                        // Release the FileIndex lock before calling save_*
                        // helpers below — they re-acquire it via
                        // `get_root_index(root)`, which would deadlock on the
                        // std::sync::Mutex held by `idx` here.
                        drop(idx);

                        // --- Native rules (full re-run, replace cache) ---
                        let native_issues: Vec<normalize_output::diagnostics::Issue> = {
                            let root_owned = root.to_path_buf();
                            let rules_config = config.rules.clone();
                            let walk_config = config.walk.clone();
                            let handle = self.runtime_handle.clone();
                            std::thread::Builder::new()
                                .stack_size(64 * 1024 * 1024)
                                .spawn(move || {
                                    handle.block_on(Self::run_native_rules(
                                        &root_owned,
                                        &rules_config,
                                        &walk_config,
                                    ))
                                })
                                .expect("failed to spawn native refresh thread")
                                .join()
                                .expect("native refresh thread panicked")
                        };

                        tracing::info!(
                            root = ?root,
                            syntax = syntax_issues.len(),
                            fact = fact_issues.len(),
                            native = native_issues.len(),
                            affected = affected_vec.len(),
                            "incremental diagnostics cache update complete"
                        );

                        // Persist to SQLite and drop Vecs immediately.
                        self.save_diagnostics_to_index(
                            root,
                            "syntax",
                            &syntax_issues,
                            &config_hash,
                        );
                        self.save_diagnostics_to_index(root, "fact", &fact_issues, &config_hash);
                        self.save_diagnostics_to_index(
                            root,
                            "native",
                            &native_issues,
                            &config_hash,
                        );
                        // Build "all" combined blob for fast no-filter hot path.
                        self.save_all_blob(
                            root,
                            &syntax_issues,
                            &fact_issues,
                            &native_issues,
                            &config_hash,
                        );
                        // Per-file table + JSON mirror for ephemeral consumers.
                        let delta = self.save_per_file_diagnostics(
                            root,
                            &syntax_issues,
                            &fact_issues,
                            &native_issues,
                            &config_hash,
                        );
                        self.write_json_mirror(root, &syntax_issues, &fact_issues, &native_issues);
                        drop(syntax_issues);
                        drop(fact_issues);
                        drop(native_issues);
                        drop(affected_vec);

                        self.broadcast_diagnostics_delta(root, delta);

                        {
                            let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(watched) = roots.get_mut(root) {
                                watched.primed = true;
                                watched.last_refresh = Instant::now();
                            }
                        }

                        // Broadcast index-refresh event. SendError means no
                        // active subscribers -- that is fine.
                        let _ = self.event_tx.send(Event::IndexRefreshed {
                            root: root.to_string_lossy().into_owned(),
                            files: changed.len() as u64,
                        });
                    }
                    Ok(_changed) => {
                        // No files changed — just update last_refresh timestamp.
                        let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(watched) = roots.get_mut(root) {
                            watched.last_refresh = Instant::now();
                        }
                    }
                    Err(e) => {
                        eprintln!("Refresh error for {:?}: {}", root, e);
                    }
                }
            } // idx lock released here — save_diagnostics_to_index may re-lock below
        }

        /// Re-run only native rules for a root and persist the results to SQLite.
        /// Called when `.git/index` changes (e.g. after `git add`) so that rules
        /// that read staged state (stale-summary, etc.) see fresh results without
        /// triggering a full index rebuild.
        fn refresh_native_rules(&self, root: &Path) {
            let config = NormalizeConfig::load(root);
            let root_owned = root.to_path_buf();
            let rules_config = config.rules.clone();
            let walk_config = config.walk.clone();
            let handle = self.runtime_handle.clone();

            let new_native_issues: Vec<normalize_output::diagnostics::Issue> =
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        handle.block_on(Self::run_native_rules(
                            &root_owned,
                            &rules_config,
                            &walk_config,
                        ))
                    })
                    .expect("failed to spawn native-rules refresh thread")
                    .join()
                    .expect("native-rules refresh thread panicked");

            tracing::info!(
                root = ?root,
                native = new_native_issues.len(),
                ".git/index refresh: native rules updated"
            );

            // Spin detection for the `.git/index` path. Records this refresh into
            // the per-root density window and flags a backoff if density is high
            // while the coalescing queue is not draining. We still persist this
            // refresh's results — backoff only suppresses *future* dirty-marks for
            // the root (see the dispatch loop's `.git/index` branch); indexing is
            // never silently dropped.
            self.record_native_refresh_and_detect_spin(root, Instant::now());

            // Persist to SQLite and drop immediately.
            let config_hash = compute_config_hash(root);
            self.save_diagnostics_to_index(root, "native", &new_native_issues, &config_hash);
            drop(new_native_issues);
            // Rebuild "all" blob from existing per-engine blobs.
            self.rebuild_all_blob(root);
            // Rebuild per-file table + JSON mirror from per-engine blobs.
            self.rebuild_per_file_diagnostics(root);
        }

        /// Tier 2 surgical config invalidation: re-run only the affected rules.
        ///
        /// Called when [`ConfigDiff`] has a non-empty `rules_to_rerun` set but
        /// `requires_full_reprime()` is false (i.e. `[walk] exclude` didn't
        /// change, so the file set is the same). For each affected rule ID:
        ///
        /// 1. Re-run the rule through whichever engines can handle it (syntax,
        ///    fact, native) using the existing per-engine filter_ids path.
        /// 2. Load the current per-engine blob, strip findings for the affected
        ///    rule IDs, merge the new findings.
        /// 3. Save updated per-engine blobs, rebuild the "all" blob and per-file
        ///    rows, broadcast a `DiagnosticsUpdated` event.
        fn surgical_rerun_rules(
            &self,
            root: &Path,
            rules_to_rerun: &HashSet<String>,
            new_rules: normalize_rules_config::RulesConfig,
            new_walk: normalize_rules_config::WalkConfig,
            new_scm_hashes: HashMap<PathBuf, [u8; 32]>,
        ) {
            tracing::info!(
                root = ?root,
                rules = rules_to_rerun.len(),
                "config reload: Tier 2 surgical per-rule re-eval"
            );

            let config_hash = compute_config_hash(root);

            // Re-run syntax rules filtered to the affected rule IDs.
            let new_syntax_for_rules: Vec<normalize_output::diagnostics::Issue> = {
                let root_owned = root.to_path_buf();
                let rules_config = new_rules.clone();
                let walk_config = new_walk.clone();
                let filter_ids = rules_to_rerun.clone();
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        let debug_flags = normalize_syntax_rules::DebugFlags::default();
                        let path_filter = normalize_rules_config::PathFilter::default();
                        let findings = normalize_rules::cmd_rules::run_syntax_rules(
                            &root_owned,
                            &root_owned,
                            None,
                            None,
                            Some(&filter_ids),
                            &rules_config,
                            &debug_flags,
                            None,
                            &path_filter,
                            &walk_config,
                        );
                        findings
                            .iter()
                            .map(|f| normalize_rules::finding_to_issue(f, &root_owned))
                            .collect()
                    })
                    .expect("failed to spawn Tier 2 syntax thread")
                    .join()
                    .expect("Tier 2 syntax thread panicked")
            };

            // Re-run fact rules filtered to the affected rule IDs.
            let new_fact_for_rules: Vec<normalize_output::diagnostics::Issue> = {
                let root_owned = root.to_path_buf();
                let rules_config = new_rules.clone();
                let filter_ids = rules_to_rerun.clone();
                let handle = self.runtime_handle.clone();
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        let diagnostics =
                            handle.block_on(normalize_rules::collect_fact_diagnostics_incremental(
                                &root_owned,
                                &rules_config,
                                Some(&filter_ids),
                                None,
                                None,
                            ));
                        diagnostics
                            .iter()
                            .map(normalize_rules::abi_diagnostic_to_issue)
                            .collect()
                    })
                    .expect("failed to spawn Tier 2 fact thread")
                    .join()
                    .expect("Tier 2 fact thread panicked")
            };

            // Re-run native rules and retain only the affected rule IDs.
            let new_native_for_rules: Vec<normalize_output::diagnostics::Issue> = {
                let root_owned = root.to_path_buf();
                let rules_config = new_rules.clone();
                let walk_config = new_walk.clone();
                let handle = self.runtime_handle.clone();
                let filter_ids = rules_to_rerun.clone();
                std::thread::Builder::new()
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        let mut all = handle.block_on(Self::run_native_rules(
                            &root_owned,
                            &rules_config,
                            &walk_config,
                        ));
                        all.retain(|i| filter_ids.contains(&i.rule_id));
                        all
                    })
                    .expect("failed to spawn Tier 2 native thread")
                    .join()
                    .expect("Tier 2 native thread panicked")
            };

            tracing::debug!(
                root = ?root,
                syntax = new_syntax_for_rules.len(),
                fact = new_fact_for_rules.len(),
                native = new_native_for_rules.len(),
                "Tier 2: new findings for affected rules"
            );

            // For each engine blob: load existing, drop findings for the
            // affected rule IDs, merge new findings, save.
            let get_idx = || self.get_root_index(root);

            for (engine, new_for_rules) in [
                ("syntax", &new_syntax_for_rules),
                ("fact", &new_fact_for_rules),
                ("native", &new_native_for_rules),
            ] {
                let Some(idx_arc) = get_idx() else { continue };
                let mut existing: Vec<normalize_output::diagnostics::Issue> = {
                    let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                    match tokio::task::block_in_place(|| {
                        self.runtime_handle
                            .block_on(idx.load_diagnostics_blob(engine, &config_hash))
                    }) {
                        Ok(Some(blob)) => rkyv::from_bytes::<
                            Vec<normalize_output::diagnostics::Issue>,
                            rkyv::rancor::Error,
                        >(&blob)
                        .unwrap_or_default(),
                        _ => Vec::new(),
                    }
                };
                // Drop findings for rules that just re-ran.
                existing.retain(|i| !rules_to_rerun.contains(&i.rule_id));
                // Merge new findings.
                existing.extend_from_slice(new_for_rules);
                self.save_diagnostics_to_index(root, engine, &existing, &config_hash);
            }

            // Rebuild derived views.
            self.rebuild_all_blob(root);
            let delta = {
                // Rebuild per-file table from the updated per-engine blobs.
                // We delegate to rebuild_per_file_diagnostics which re-reads
                // the blobs we just wrote and diffs against the existing rows.
                let Some(idx_arc) = get_idx() else {
                    return;
                };
                let mut syntax: Vec<normalize_output::diagnostics::Issue> = Vec::new();
                let mut fact: Vec<normalize_output::diagnostics::Issue> = Vec::new();
                let mut native: Vec<normalize_output::diagnostics::Issue> = Vec::new();
                for (eng, sink) in [
                    ("syntax", &mut syntax),
                    ("fact", &mut fact),
                    ("native", &mut native),
                ] {
                    let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                    if let Ok(Some(blob)) = tokio::task::block_in_place(|| {
                        self.runtime_handle
                            .block_on(idx.load_diagnostics_blob(eng, &config_hash))
                    }) && let Ok(issues) = rkyv::from_bytes::<
                        Vec<normalize_output::diagnostics::Issue>,
                        rkyv::rancor::Error,
                    >(&blob)
                    {
                        *sink = issues;
                    }
                }
                let d = self.save_per_file_diagnostics(root, &syntax, &fact, &native, &config_hash);
                self.write_json_mirror(root, &syntax, &fact, &native);
                d
            };

            // Update the cached config snapshot.
            {
                let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(w) = roots.get_mut(root) {
                    w.cached_rules = Arc::new(new_rules);
                    w.cached_walk = Arc::new(new_walk);
                    // Update the .scm hash snapshot so the next reload can
                    // diff against the hashes that produced the current blobs.
                    w.cached_scm_hashes = new_scm_hashes;
                    w.serve_filter_pending = false;
                }
            }

            self.broadcast_diagnostics_delta(root, delta);
        }

        /// Handle a `.normalize/config.toml` (or `.normalize/rules/**`) change.
        ///
        /// Computes a [`ConfigDiff`] between the cached snapshot (what the
        /// persisted blobs were produced under) and the freshly-loaded config,
        /// then routes to the cheapest correct strategy:
        ///
        /// - **Filter-only diff** (severity / allow / `enabled = false`):
        ///   update the cached config and set `serve_filter_pending` so the
        ///   serve paths drop / re-severity cached findings on the next
        ///   `RunRules`. No blob clear, no reprime. Broadcasts an
        ///   `IndexRefreshed { files: 0 }` so subscribers know the view
        ///   changed; a follow-up `DiagnosticsUpdated` is *not* emitted by
        ///   this path because the per-file blobs on disk are unchanged —
        ///   subscribers re-pull on their own.
        /// - **No-op diff** (config bytes changed but no observable effect,
        ///   e.g. only a comment): nothing to do.
        /// - **Per-rule re-eval** (a rule was newly enabled, its `extra`
        ///   threshold changed, etc.): re-run only the affected rules through
        ///   each engine, splice updated findings into the stored blobs, and
        ///   broadcast `DiagnosticsUpdated`. No full reprime needed.
        /// - **Full reprime** (`[walk] exclude` changed — file set differs):
        ///   clear blobs and reprime from scratch.
        fn reload_config_and_reprime(&self, root: &Path) {
            // Snapshot old config + load new config under the lock so the
            // diff sees a consistent view.
            let new_config = NormalizeConfig::load(root);
            let new_rules = new_config.rules.clone();
            let new_walk = new_config.walk.clone();

            let (old_rules, old_walk, old_scm_hashes) = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                match roots.get(root) {
                    Some(w) => (
                        w.cached_rules.clone(),
                        w.cached_walk.clone(),
                        w.cached_scm_hashes.clone(),
                    ),
                    None => return, // root not watched — nothing to do
                }
            };

            // Compute fresh .scm hashes and diff against the cached snapshot
            // to find which custom rule files changed.
            let new_scm_hashes = hash_scm_files(root);
            let scm_changed_rule_ids: HashSet<String> = {
                let mut ids = HashSet::new();
                // Files that were added or changed.
                for (path, new_hash) in &new_scm_hashes {
                    if old_scm_hashes.get(path) != Some(new_hash)
                        && let Some(id) = scm_path_to_rule_id(path)
                    {
                        ids.insert(id);
                    }
                }
                // Files that were removed.
                for path in old_scm_hashes.keys() {
                    if !new_scm_hashes.contains_key(path)
                        && let Some(id) = scm_path_to_rule_id(path)
                    {
                        ids.insert(id);
                    }
                }
                ids
            };

            let mut diff = normalize_rules_config::ConfigDiff::compute(
                &old_rules, &new_rules, &old_walk, &new_walk,
            );

            // Union .scm-derived rule IDs into the config diff so they route
            // through Tier 2 (surgical re-run) rather than Tier 3 (full reprime).
            if !scm_changed_rule_ids.is_empty() {
                tracing::info!(
                    root = ?root,
                    scm_rules = scm_changed_rule_ids.len(),
                    "config reload: .scm files changed — adding to rules_to_rerun"
                );
                diff.rules_to_rerun.extend(scm_changed_rule_ids);
            }

            if diff.is_empty() {
                // The config bytes changed (notify fired) but nothing the
                // engines care about did (e.g. comment edit, whitespace).
                // Refresh the snapshot in case the bytes-equal-but-not-eq
                // case introduced ordering noise, then announce the reload
                // so subscribers (LSPs, the daemon-push test) still see the
                // signal that some `.normalize/config.toml` edit landed.
                let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(w) = roots.get_mut(root) {
                    w.cached_rules = Arc::new(new_rules);
                    w.cached_walk = Arc::new(new_walk);
                    w.cached_scm_hashes = new_scm_hashes;
                }
                drop(roots);
                tracing::debug!(root = ?root, "config reload: no observable change");
                let _ = self.event_tx.send(Event::IndexRefreshed {
                    root: root.to_string_lossy().into_owned(),
                    files: 0,
                });
                return;
            }

            if diff.is_filter_only() {
                tracing::info!(
                    root = ?root,
                    severities = diff.severities_changed,
                    allow_lists = diff.allow_lists_changed,
                    disabled = diff.rules_disabled.len(),
                    "config reload: filter-only (no reprime)"
                );
                // Swap in the new cached config + flip the pending-filter
                // flag. Subsequent `RunRules` calls will re-filter cached
                // blobs at serve time.
                {
                    let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(w) = roots.get_mut(root) {
                        w.cached_rules = Arc::new(new_rules);
                        w.cached_walk = Arc::new(new_walk);
                        w.cached_scm_hashes = new_scm_hashes;
                        w.serve_filter_pending = true;
                    }
                }
                // Announce the reload so subscribers re-pull. files: 0 keeps
                // the existing test contract that this signal accompanies a
                // config edit.
                let _ = self.event_tx.send(Event::IndexRefreshed {
                    root: root.to_string_lossy().into_owned(),
                    files: 0,
                });
                return;
            }

            // Tier 2: per-rule re-eval — the file set hasn't changed but one
            // or more rules need to re-run (including any .scm file edits
            // detected above). Splice updated findings into the existing blobs
            // without clearing everything.
            if !diff.requires_full_reprime() {
                self.surgical_rerun_rules(
                    root,
                    &diff.rules_to_rerun,
                    new_rules,
                    new_walk,
                    new_scm_hashes,
                );
                let _ = self.event_tx.send(Event::IndexRefreshed {
                    root: root.to_string_lossy().into_owned(),
                    files: 0,
                });
                return;
            }

            // Tier 3 (full reprime): `[walk] exclude` changed — file set
            // differs, must clear blobs and reprime from scratch.
            tracing::info!(
                root = ?root,
                rerun = diff.rules_to_rerun.len(),
                walk = diff.walk_exclude_changed,
                "config reload: full reprime"
            );

            // Mark un-primed and drop cached blobs so a concurrent `RunRules`
            // can't surface findings under the old config between now and
            // reprime completion.
            {
                let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(w) = roots.get_mut(root) {
                    w.primed = false;
                }
            }

            if let Some(idx_arc) = self.get_root_index(root) {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = tokio::task::block_in_place(|| {
                    self.runtime_handle.block_on(idx.clear_all_diagnostics())
                }) {
                    tracing::warn!(
                        root = ?root,
                        "failed to clear diagnostics during config reload: {}",
                        e
                    );
                }
                drop(idx);
            }

            let _ = self.event_tx.send(Event::IndexRefreshed {
                root: root.to_string_lossy().into_owned(),
                files: 0,
            });

            self.prime_diagnostics_cache(root);
        }
    }

    /// Run the global daemon server.
    pub async fn run_daemon() -> Result<i32, Box<dyn std::error::Error>> {
        let socket_path = global_socket_path();

        // Ensure config directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Acquire exclusive lock -- only one daemon process can run at a time.
        // The lock is held for the lifetime of the process (leaked intentionally).
        let lock = acquire_daemon_lock().map_err(|e| {
            eprintln!("Cannot start daemon: {}", e);
            e
        })?;
        // Leak the File so the flock is held until process exit
        std::mem::forget(lock);

        // Safe to remove socket now -- we hold the lock
        let _ = std::fs::remove_file(&socket_path);

        // Channel for full refresh requests from file watchers
        let (refresh_tx, refresh_rx) = channel::<PathBuf>();
        // Coalescing queue for native-rules-only refresh requests triggered by
        // .git/index changes. Unlike an unbounded channel, this collapses repeated
        // requests for the same root into a single pending refresh (see DirtyRoots).
        let native_dirty = Arc::new(DirtyRoots::new());
        // Channel for config-reload requests triggered by `.normalize/config.toml`
        // or `.normalize/rules/**` changes.
        let (config_reload_tx, config_reload_rx) = channel::<PathBuf>();

        // Create shared file watcher that handles events for all roots
        let (notify_tx, notify_rx) = channel::<notify::Result<notify::Event>>();
        let shared_watcher = RecommendedWatcher::new(notify_tx, Config::default())
            .expect("failed to create shared file watcher");

        let server = Arc::new(DaemonServer::new(
            refresh_tx,
            native_dirty.clone(),
            config_reload_tx,
            tokio::runtime::Handle::current(),
            shared_watcher,
        ));

        // GC any roots whose path has disappeared since last run. No-op today
        // (roots aren't persisted across restarts) but preserves the invariant
        // that the daemon never holds watchers for non-existent paths.
        server.gc_dead_roots();

        // Spawn single dispatch thread to handle all file-watcher events across all roots.
        // This replaces the previous per-root pair of watcher threads, saving ~3 OS threads
        // per root.
        let server_dispatch = server.clone();
        std::thread::spawn(move || {
            let debounce = Duration::from_millis(500);
            let git_debounce = Duration::from_millis(200);
            let config_debounce = Duration::from_millis(500);
            let mut last_refresh: HashMap<PathBuf, Instant> = HashMap::new();
            let mut last_native: HashMap<PathBuf, Instant> = HashMap::new();
            let mut last_config_reload: HashMap<PathBuf, Instant> = HashMap::new();

            for result in notify_rx {
                let event = match result {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("notify error: {}", e);
                        continue;
                    }
                };

                // Collect dispatch targets under a brief lock, then release before sending
                let mut to_event: Vec<(PathBuf, PathBuf)> = Vec::new(); // (path, root)
                let mut to_refresh: Vec<PathBuf> = Vec::new();
                let mut to_native: Vec<PathBuf> = Vec::new();
                let mut to_config_reload: Vec<PathBuf> = Vec::new();
                // Context file changes: (file_path, context_index_arc)
                let mut to_context_update: Vec<(PathBuf, Arc<Mutex<ContextIndex>>)> = Vec::new();
                // Directories created at runtime that need to be added to the
                // watcher so their contents are observable. Walked through
                // `gitignore_walk` (so e.g. a freshly-created `target/` is
                // skipped) after the roots lock is released.
                let mut to_watch_new_dir: Vec<(PathBuf, PathBuf)> = Vec::new(); // (new_dir, root)
                let is_create_event = matches!(
                    event.kind,
                    notify::EventKind::Create(notify::event::CreateKind::Folder)
                        | notify::EventKind::Create(notify::event::CreateKind::Any)
                );

                {
                    let mut roots = server_dispatch
                        .roots
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    // Snapshot keys so we can mutate `roots` (spin backoff state)
                    // inside the loop without holding an immutable borrow.
                    let root_keys: Vec<PathBuf> = roots.keys().cloned().collect();
                    for path in &event.paths {
                        // Find the root this path belongs to
                        let root = root_keys.iter().find(|r| path.starts_with(r.as_path()));
                        let Some(root) = root else { continue };
                        let root = root.clone();
                        let root = &root;

                        // Classify the path. Config and rule-definition paths
                        // route to a config-reload (clear blobs + full reprime).
                        // Other `.normalize/` internal paths (SQLite WAL,
                        // diagnostics.json mirror, daemon socket dir) are
                        // produced *by* the daemon and must be skipped to avoid
                        // a feedback loop. Everything else is a source-file
                        // change that drives the normal refresh path.
                        let normalize_dir = root.join(".normalize");
                        let config_path = normalize_dir.join("config.toml");
                        let rules_dir = normalize_dir.join("rules");
                        let context_dir = normalize_dir.join("context");
                        let is_config_change = path == &config_path || path.starts_with(&rules_dir);
                        let is_context_change = path.starts_with(&context_dir)
                            && path.extension().and_then(|e| e.to_str()) == Some("md");
                        let is_internal_normalize_state = !is_config_change
                            && !is_context_change
                            && path.starts_with(&normalize_dir);
                        if is_internal_normalize_state {
                            continue;
                        }

                        if is_context_change {
                            if let Some(ctx_arc) = roots.get(root).map(|w| w.context_index.clone())
                            {
                                to_context_update.push((path.clone(), ctx_arc));
                            }
                            continue;
                        }

                        if is_config_change {
                            let last = last_config_reload
                                .entry(root.clone())
                                .or_insert(Instant::now() - config_debounce * 2);
                            if last.elapsed() >= config_debounce {
                                to_config_reload.push(root.clone());
                                *last = Instant::now();
                            }
                            // Don't broadcast FileChanged for config edits — they're
                            // not source-file changes from a subscriber's perspective.
                            continue;
                        }

                        to_event.push((path.clone(), root.clone()));

                        // If the event is a directory-create, schedule the new
                        // subtree for registration with the watcher (after we
                        // drop the roots lock). Without this, files created
                        // inside the new directory would be invisible — we
                        // only watch directories, NonRecursively.
                        if is_create_event && path.is_dir() {
                            to_watch_new_dir.push((path.clone(), root.clone()));
                        }

                        // Spin backoff (per-root): while a root is flagged as
                        // spinning, skip dispatching *any* refresh for it (full or
                        // native) so the feedback loop can't keep firing. When the
                        // cooldown expires, clear the flag and log the resume. This
                        // applies to both the source-file spin signal
                        // (`record_refresh_and_detect_spin`) and the `.git/index`
                        // spin signal (`record_native_refresh_and_detect_spin`).
                        let now = Instant::now();
                        let backed_off = match roots.get_mut(root) {
                            Some(w) => match w.spin_flagged_until {
                                Some(until) if now < until => true,
                                Some(_) => {
                                    w.spin_flagged_until = None;
                                    tracing::info!(
                                        root = %root.display(),
                                        "spin backoff lifted; resuming refreshes",
                                    );
                                    false
                                }
                                None => false,
                            },
                            None => false,
                        };
                        if backed_off {
                            continue;
                        }

                        // Check if this is a .git/index change (triggers native-rules refresh)
                        let git_index = root.join(".git").join("index");
                        if path == &git_index {
                            let last = last_native
                                .entry(root.clone())
                                .or_insert(Instant::now() - git_debounce * 2);
                            if last.elapsed() >= git_debounce {
                                to_native.push(root.clone());
                                *last = Instant::now();
                            }
                        } else {
                            let last = last_refresh
                                .entry(root.clone())
                                .or_insert(Instant::now() - debounce * 2);
                            if last.elapsed() >= debounce {
                                to_refresh.push(root.clone());
                                *last = Instant::now();
                            }
                        }
                    }
                } // lock released before sending

                // Register newly-created directories with the watcher. We
                // re-fetch the cached walk config under the roots lock (briefly)
                // so that `[walk] exclude` is honored — a freshly-created
                // `target/` should not be watched. The walker also descends
                // into the new directory itself, picking up any nested dirs
                // that were created in the same batch.
                for (new_dir, root) in to_watch_new_dir {
                    let walk_config = {
                        let roots = server_dispatch
                            .roots
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        roots.get(&root).map(|w| w.cached_walk.clone())
                    };
                    let Some(walk_config) = walk_config else {
                        continue;
                    };
                    // Skip if the new dir itself is excluded — `gitignore_walk`
                    // handles this when walking into it, but checking up front
                    // avoids spawning the walker for an excluded `target/`.
                    let rel = new_dir.strip_prefix(&root).unwrap_or(&new_dir);
                    if !rel.as_os_str().is_empty() && walk_config.is_excluded_path(&root, rel, true)
                    {
                        continue;
                    }
                    let mut watcher = server_dispatch
                        .watcher
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let added = watch_tree_non_recursive(&mut watcher, &new_dir, &walk_config);
                    if added > 0 {
                        tracing::debug!(
                            path = %new_dir.display(),
                            dirs = added,
                            "added newly-created directory to file watcher",
                        );
                    }
                }

                // Broadcast un-debounced file-change events
                for (path, root) in to_event {
                    let _ = server_dispatch.event_tx.send(Event::FileChanged {
                        path: path.to_string_lossy().into_owned(),
                        root: root.to_string_lossy().into_owned(),
                    });
                }
                for root in to_refresh {
                    let _ = server_dispatch.refresh_tx.send(root);
                }
                for root in to_native {
                    server_dispatch.native_dirty.mark(root);
                }
                for root in to_config_reload {
                    let _ = server_dispatch.config_reload_tx.send(root);
                }
                // Update the in-memory context index for changed context files.
                // Done inline (no channel needed) — loading a single .md file is
                // fast (<1ms) and doesn't block the event loop.
                for (path, ctx_arc) in to_context_update {
                    let mut ctx_index = ctx_arc.lock().unwrap_or_else(|e| e.into_inner());
                    if path.exists() {
                        ctx_index.load_file(&path);
                    } else {
                        ctx_index.remove_file(&path);
                    }
                    tracing::debug!(path = %path.display(), "context index updated");
                }
            }
        });

        // Spawn full-refresh handler
        let server_refresh = server.clone();
        std::thread::spawn(move || {
            for root in refresh_rx {
                server_refresh.refresh_root(&root);
            }
        });

        // Spawn native-rules-only refresh handler (for .git/index changes).
        // Drains the coalescing `DirtyRoots` set: each wake yields the set of
        // distinct roots dirtied since the last drain, and each is refreshed
        // exactly once from its current on-disk state. Repeated `.git/index`
        // events for the same root collapse to a single refresh, so the backlog
        // is bounded by the number of watched roots rather than by git churn.
        let server_native = server.clone();
        let native_dirty_consumer = native_dirty.clone();
        std::thread::spawn(move || {
            while let Some(roots) = native_dirty_consumer.drain_blocking() {
                for root in roots {
                    server_native.refresh_native_rules(&root);
                }
            }
        });

        // Spawn config-reload handler (for `.normalize/config.toml` and
        // `.normalize/rules/**` changes). Drops cached diagnostic blobs and
        // triggers a full reprime against the freshly-loaded config.
        let server_config = server.clone();
        std::thread::spawn(move || {
            for root in config_reload_rx {
                server_config.reload_config_and_reprime(&root);
            }
        });

        // Start socket server
        let listener = UnixListener::bind(&socket_path)?;
        eprintln!("Daemon listening on {}", socket_path.display());

        loop {
            let (stream, _) = listener.accept().await?;
            let server = server.clone();

            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut reader = tokio::io::BufReader::new(reader);

                // Detect protocol by peeking at the first byte.
                // 0x01 (SOH) = rkyv binary mode; anything else = JSON mode.
                let mut first = [0u8; 1];
                if reader.read_exact(&mut first).await.is_err() {
                    return;
                }

                if first[0] == 0x01 {
                    handle_rkyv_connection(&server, &mut reader, &mut writer).await;
                    return;
                }

                // JSON mode: reconstruct the first line from the already-read byte.
                let mut line = String::new();
                if first[0] != b'\n' {
                    line.push(first[0] as char);
                    // Read the rest of the first line.
                    reader.read_line(&mut line).await.unwrap_or(0);
                }
                if !line.trim().is_empty() {
                    handle_json_line(&server, &line, &mut writer).await;
                }
                line.clear();

                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    if !line.trim().is_empty() {
                        handle_json_line(&server, &line, &mut writer).await;
                    }
                    line.clear();
                }
            });
        }
    }

    /// Handle one JSON request line in the legacy JSON IPC protocol.
    async fn handle_json_line(
        server: &Arc<DaemonServer>,
        line: &str,
        writer: &mut tokio::net::unix::OwnedWriteHalf,
    ) {
        use tokio::io::AsyncWriteExt;
        match serde_json::from_str::<Request>(line) {
            Ok(Request::Shutdown) => {
                let resp = server.handle_request(Request::Shutdown);
                // normalize-syntax-allow: rust/unwrap-in-impl - Response is always JSON-serializable
                let resp_str = serde_json::to_string(&resp).unwrap();
                let _ = writer.write_all(resp_str.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
                let _ = writer.flush().await;
                // Graceful cleanup before exit. The OS releases the flock on
                // process exit regardless (flock is fd/process-scoped, not
                // file-existence-scoped — startup tolerates a leftover lock file
                // from a crashed daemon), but removing the lock + socket files
                // here keeps the config dir clean and avoids confusing users who
                // inspect `~/.config/normalize/`. `std::process::exit` skips
                // destructors, so this explicit removal is the only chance to do
                // it on a graceful stop.
                let _ = std::fs::remove_file(global_socket_path());
                let _ = std::fs::remove_file(daemon_lock_path());
                std::process::exit(0);
            }
            Ok(Request::Subscribe { root }) => {
                // Ensure the root is being watched (add if not already).
                if let Some(r) = root {
                    server.add_root(r);
                }
                // Subscribe to the broadcast channel and stream events
                // until the client disconnects or the daemon shuts down.
                let mut rx = server.event_tx.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            // normalize-syntax-allow: rust/unwrap-in-impl - Event is always JSON-serializable
                            let json = serde_json::to_string(&event).unwrap();
                            if writer.write_all(json.as_bytes()).await.is_err()
                                || writer.write_all(b"\n").await.is_err()
                            {
                                // Client disconnected
                                return;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // Subscriber fell behind -- log and continue.
                            eprintln!("Subscriber lagged, dropped {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Daemon is shutting down
                            return;
                        }
                    }
                }
            }
            Ok(req) => {
                let response = server.handle_request(req);
                // normalize-syntax-allow: rust/unwrap-in-impl - Response is always JSON-serializable
                let resp_str = serde_json::to_string(&response).unwrap();
                let _ = writer.write_all(resp_str.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
            }
            Err(e) => {
                let response = Response::err(&format!("Invalid request: {}", e));
                // normalize-syntax-allow: rust/unwrap-in-impl - Response is always JSON-serializable
                let resp_str = serde_json::to_string(&response).unwrap();
                let _ = writer.write_all(resp_str.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
            }
        }
    }

    /// Handle one connection in rkyv binary IPC mode.
    ///
    /// Protocol: client sends `[0x01][json_request_bytes][\n]` (magic byte already
    /// consumed by the caller).  Daemon responds with `[type_byte][4-byte LE len][payload]`
    /// where `type_byte` is `0x01` (rkyv payload) or `0x00` (JSON error string).
    async fn handle_rkyv_connection(
        server: &Arc<DaemonServer>,
        reader: &mut tokio::io::BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: &mut tokio::net::unix::OwnedWriteHalf,
    ) {
        use tokio::io::AsyncWriteExt;

        let mut line = String::new();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            return;
        }

        // Parse JSON request (same schema as the JSON protocol).
        let raw_response = match serde_json::from_str::<Request>(&line) {
            Ok(Request::RunRules {
                root,
                filter_ids,
                filter_rule,
                engine,
                filter_files,
            }) => server.run_rules_raw(root, filter_ids, filter_rule, engine, filter_files),
            Ok(Request::Subscribe { root }) => {
                // Binary subscribe: stream rkyv-encoded events as
                // [type_byte=0x01][4-byte LE len][rkyv payload] frames until
                // the client disconnects.
                if let Some(r) = root {
                    server.add_root(r);
                }
                let mut rx = server.event_tx.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            let blob = match rkyv::to_bytes::<rkyv::rancor::Error>(&event) {
                                Ok(b) => b,
                                Err(e) => {
                                    tracing::warn!("rkyv serialize Event: {}", e);
                                    continue;
                                }
                            };
                            if writer.write_all(&[0x01]).await.is_err()
                                || writer
                                    .write_all(&(blob.len() as u32).to_le_bytes())
                                    .await
                                    .is_err()
                                || writer.write_all(&blob).await.is_err()
                            {
                                return;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("Subscriber lagged, dropped {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => return,
                    }
                }
            }
            Ok(_) => RawResponse::Error("rkyv mode only supports run_rules and subscribe".into()),
            Err(e) => RawResponse::Error(format!("invalid request: {e}")),
        };

        let (type_byte, payload): (u8, Vec<u8>) = match raw_response {
            RawResponse::Frame(b) => (0x01, b),
            RawResponse::Error(msg) => (0x00, msg.into_bytes()),
        };

        let _ = writer.write_all(&[type_byte]).await;
        let _ = writer
            .write_all(&(payload.len() as u32).to_le_bytes())
            .await;
        let _ = writer.write_all(&payload).await;
    }

    /// Client for communicating with the global daemon.
    pub struct DaemonClient {
        socket_path: PathBuf,
    }

    impl DaemonClient {
        /// Construct a client using the default socket path.
        ///
        /// Resolves `NORMALIZE_DAEMON_CONFIG_DIR` (via [`global_socket_path`])
        /// **once** at construction and stores the result. Subsequent method
        /// calls use the captured path, so changing the env var afterwards has
        /// no effect on this client. This makes the client safe to use across
        /// threads, in tests, in the LSP, and in library embedders without
        /// racing on a process-global env var.
        pub fn new() -> Self {
            Self::with_socket_path(global_socket_path())
        }

        /// Construct a client with an explicit socket path.
        ///
        /// Useful for tests, LSPs talking to multiple workspaces, and library
        /// embedders that want full control over which daemon they target —
        /// no env-var read involved.
        pub fn with_socket_path(socket_path: PathBuf) -> Self {
            Self { socket_path }
        }

        pub fn is_available(&self) -> bool {
            if !self.socket_path.exists() {
                return false;
            }
            self.send(&Request::Status).is_ok()
        }

        /// Ensure daemon is running, starting it if necessary.
        /// Uses flock to prevent concurrent spawn races.
        pub fn ensure_running(&self) -> bool {
            if self.is_available() {
                return true;
            }

            // Take the spawn lock to prevent multiple clients from spawning
            // concurrently. If we can't get the lock, another client is already
            // spawning -- just wait for the socket to appear.
            match try_flock(&spawn_lock_path()) {
                Ok(_lock) => {
                    // Re-check after acquiring lock -- another client may have
                    // finished spawning while we waited.
                    if self.is_available() {
                        return true;
                    }
                    // Clean up stale socket before spawning
                    let _ = std::fs::remove_file(&self.socket_path);
                    // Lock is released when _lock drops (after spawn + socket wait)
                    self.start_daemon().is_ok()
                }
                Err(_) => {
                    // Spawn lock held -- another client is spawning. Wait for socket.
                    self.wait_for_socket()
                }
            }
        }

        /// Wait for the daemon socket to become available.
        fn wait_for_socket(&self) -> bool {
            for _ in 0..30 {
                if self.is_available() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            false
        }

        fn start_daemon(&self) -> Result<(), String> {
            use std::process::{Command, Stdio};

            // Ensure config directory exists
            if let Some(parent) = self.socket_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create config directory: {}", e))?;
            }

            let current_exe =
                std::env::current_exe().map_err(|e| format!("Failed to get executable: {}", e))?;

            // Mark the child as an auto-started daemon so it routes tracing
            // output to the daemon log file. stderr is nulled below, so without
            // this the daemon's WARN/ERROR (including spin-loop detection) would
            // vanish — which is exactly why two spin loops went unnoticed.
            Command::new(&current_exe)
                .arg("daemon")
                .arg("run")
                .env(DAEMON_LOG_ENV, "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Failed to spawn daemon: {}", e))?;

            // Fire-and-forget: don't block waiting for the socket. The daemon initializes
            // in the background; the *next* invocation will find it running and connect
            // instantly. Blocking here would add up to 2s to every command that triggers
            // auto-start, which defeats the purpose of the daemon entirely.
            Ok(())
        }

        pub fn send(&self, request: &Request) -> Result<Response, String> {
            use std::io::{BufRead, BufReader, Write};
            let mut stream = UnixStream::connect(&self.socket_path)
                .map_err(|e| format!("Failed to connect: {}", e))?;

            stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

            let json = serde_json::to_string(request).map_err(|e| e.to_string())?;
            stream
                .write_all(json.as_bytes())
                .map_err(|e| e.to_string())?;
            stream.write_all(b"\n").map_err(|e| e.to_string())?;

            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).map_err(|e| e.to_string())?;

            serde_json::from_str(&line).map_err(|e| e.to_string())
        }

        pub fn add_root(&self, root: &Path) -> Result<Response, String> {
            self.send(&Request::Add {
                root: root.to_path_buf(),
            })
        }

        pub fn remove_root(&self, root: &Path) -> Result<Response, String> {
            self.send(&Request::Remove {
                root: root.to_path_buf(),
            })
        }

        pub fn list_roots(&self) -> Result<Response, String> {
            self.send(&Request::List)
        }

        pub fn status(&self) -> Result<Response, String> {
            self.send(&Request::Status)
        }

        pub fn shutdown(&self) -> Result<(), String> {
            let _ = self.send(&Request::Shutdown);
            Ok(())
        }

        /// Notify the daemon that specific files have been written to disk and
        /// should be re-indexed. This is the "push" complement to inotify:
        /// callers (e.g. refactoring commands) call this immediately after
        /// writing files so the index refreshes without waiting for inotify.
        ///
        /// Non-fatal: if the daemon is not running or the root is not watched,
        /// returns `Ok(())` after logging a debug message.
        pub fn notify_files_changed(&self, root: &Path, paths: &[PathBuf]) -> Result<(), String> {
            if paths.is_empty() {
                return Ok(());
            }
            match self.send(&Request::FilesChanged {
                root: root.to_path_buf(),
                paths: paths.to_vec(),
            }) {
                Ok(_) => Ok(()),
                Err(e) => {
                    tracing::debug!("notify_files_changed: daemon unavailable ({})", e);
                    Ok(())
                }
            }
        }

        /// Run rules via the daemon, returning cached diagnostics.
        ///
        /// Uses a 5-minute timeout since the first request may trigger a full
        /// cache prime. Subsequent requests return instantly from cache.
        pub fn run_rules(
            &self,
            root: &Path,
            filter_ids: Option<Vec<String>>,
            filter_rule: Option<String>,
            engine: Option<String>,
            filter_files: Option<Vec<String>>,
        ) -> Result<Response, String> {
            use std::io::{BufRead, BufReader, Write};
            let mut stream = UnixStream::connect(&self.socket_path)
                .map_err(|e| format!("Failed to connect: {}", e))?;

            // Short timeout: only use daemon if it has warm cached results.
            // If still priming, the caller falls back to local computation.
            stream
                .set_read_timeout(Some(Duration::from_millis(500)))
                .ok();
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

            let request = Request::RunRules {
                root: root.to_path_buf(),
                filter_ids,
                filter_rule,
                engine,
                filter_files,
            };
            let json = serde_json::to_string(&request).map_err(|e| e.to_string())?;
            stream
                .write_all(json.as_bytes())
                .map_err(|e| e.to_string())?;
            stream.write_all(b"\n").map_err(|e| e.to_string())?;

            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            reader.read_line(&mut line).map_err(|e| e.to_string())?;

            serde_json::from_str(&line).map_err(|e| e.to_string())
        }

        /// Query the daemon's in-memory context index.
        ///
        /// Returns matching context blocks without any filesystem I/O.
        /// Falls back to `Err` if the daemon is not available or the root is not watched.
        pub fn query_context(
            &self,
            root: &Path,
            match_keys: Vec<(String, String)>,
            all: bool,
            dir_name: Option<String>,
        ) -> Result<Response, String> {
            let request = Request::QueryContext {
                root: root.to_path_buf(),
                match_keys,
                all,
                dir_name,
            };
            self.send(&request)
        }

        /// Subscribe to daemon events, calling `on_event` for each one.
        ///
        /// Blocks until the connection is closed or `on_event` returns `false`.
        /// If `root` is `Some`, it is automatically added to the daemon's watch list.
        pub fn watch_events(
            &self,
            root: Option<&Path>,
            mut on_event: impl FnMut(Event) -> bool,
        ) -> Result<(), String> {
            use std::io::{BufRead, BufReader, Write};
            let mut stream = UnixStream::connect(&self.socket_path)
                .map_err(|e| format!("Failed to connect: {}", e))?;

            // No read timeout -- block indefinitely waiting for events.
            stream.set_read_timeout(None).ok();
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

            let req = Request::Subscribe {
                root: root.map(|p| p.to_path_buf()),
            };
            let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
            stream
                .write_all(json.as_bytes())
                .map_err(|e| e.to_string())?;
            stream.write_all(b"\n").map_err(|e| e.to_string())?;

            let reader = BufReader::new(&stream);
            for line in reader.lines() {
                let line = line.map_err(|e| format!("Read error: {}", e))?;
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Event>(&line) {
                    Ok(event) => {
                        if !on_event(event) {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to parse daemon event: {}", e);
                    }
                }
            }

            Ok(())
        }

        /// Subscribe with binary framing. Calls `on_event` for each pushed
        /// event, deserialized from rkyv length-prefixed frames.
        ///
        /// Returns when the connection closes or `on_event` returns `false`.
        /// Compared to [`watch_events`](Self::watch_events) (JSON-line), the
        /// binary path can carry the full `Event::DiagnosticsUpdated` payload
        /// (an arbitrarily large per-file delta) without per-event JSON
        /// encode/decode cost.
        pub fn watch_events_binary(
            &self,
            root: Option<&Path>,
            mut on_event: impl FnMut(Event) -> bool,
        ) -> Result<(), String> {
            use std::io::{Read, Write};
            let mut stream = UnixStream::connect(&self.socket_path)
                .map_err(|e| format!("Failed to connect: {}", e))?;
            stream.set_read_timeout(None).ok();
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

            // Magic byte = rkyv binary mode.
            stream.write_all(&[0x01]).map_err(|e| e.to_string())?;
            let req = Request::Subscribe {
                root: root.map(|p| p.to_path_buf()),
            };
            let json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
            stream
                .write_all(json.as_bytes())
                .map_err(|e| e.to_string())?;
            stream.write_all(b"\n").map_err(|e| e.to_string())?;

            loop {
                let mut hdr = [0u8; 5];
                if stream.read_exact(&mut hdr).is_err() {
                    break;
                }
                if hdr[0] != 0x01 {
                    // 0x00 = JSON error string from the daemon -- drain and stop.
                    break;
                }
                let len = u32::from_le_bytes([hdr[1], hdr[2], hdr[3], hdr[4]]) as usize;
                let mut aligned = rkyv::util::AlignedVec::<16>::with_capacity(len);
                aligned.resize(len, 0);
                if stream.read_exact(&mut aligned[..]).is_err() {
                    break;
                }
                let event = match rkyv::from_bytes::<Event, rkyv::rancor::Error>(&aligned) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("rkyv deserialize Event: {}", e);
                        continue;
                    }
                };
                if !on_event(event) {
                    break;
                }
            }
            Ok(())
        }
    }

    impl Default for DaemonClient {
        fn default() -> Self {
            Self::new()
        }
    }

    // =========================================================================
    // Tests for per-file diagnostics + JSON mirror.
    //
    // These live inside `unix_impl` so they can construct a minimal
    // `DaemonServer` (with no real watchers/runtime) and exercise the
    // per-file storage + delta logic directly.
    // =========================================================================
    #[cfg(test)]
    mod config_hash_tests {
        use super::*;

        /// Stable across calls when nothing on disk changes.
        #[test]
        fn stable_when_inputs_unchanged() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".normalize")).unwrap();
            std::fs::write(dir.path().join(".normalize/config.toml"), "x = 1\n").unwrap();
            let h1 = compute_config_hash(dir.path());
            let h2 = compute_config_hash(dir.path());
            assert_eq!(h1, h2);
        }

        /// Editing config.toml flips the hash.
        #[test]
        fn changes_when_config_toml_changes() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".normalize")).unwrap();
            std::fs::write(dir.path().join(".normalize/config.toml"), "x = 1\n").unwrap();
            let h1 = compute_config_hash(dir.path());
            std::fs::write(dir.path().join(".normalize/config.toml"), "x = 2\n").unwrap();
            let h2 = compute_config_hash(dir.path());
            assert_ne!(h1, h2);
        }

        /// Adding a rule definition file flips the hash.
        #[test]
        fn changes_when_rules_dir_changes() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".normalize/rules")).unwrap();
            let h1 = compute_config_hash(dir.path());
            std::fs::write(dir.path().join(".normalize/rules/foo.scm"), "(_)\n").unwrap();
            let h2 = compute_config_hash(dir.path());
            assert_ne!(h1, h2);
        }

        /// Empty project (no `.normalize/` at all) is still well-defined and
        /// stable — the binary version always contributes.
        #[test]
        fn empty_project_is_stable() {
            let dir = tempfile::tempdir().unwrap();
            let h1 = compute_config_hash(dir.path());
            let h2 = compute_config_hash(dir.path());
            assert_eq!(h1, h2);
            // Sanity: matches blake3 hex digest length (64 chars).
            assert_eq!(h1.len(), 64);
        }
    }

    #[cfg(test)]
    mod per_file_tests {
        use super::*;
        use normalize_output::diagnostics::{Issue, Severity};

        fn issue(file: &str, line: usize, msg: &str) -> Issue {
            Issue {
                file: file.to_string(),
                line: Some(line),
                column: Some(1),
                end_line: None,
                end_column: None,
                rule_id: "test/rule".to_string(),
                message: msg.to_string(),
                severity: Severity::Warning,
                source: "test".to_string(),
                related: Vec::new(),
                suggestion: None,
            }
        }

        /// Construct a minimal `DaemonServer` whose only working state is the
        /// SQLite-backed `WatchedRoot.index`. Watchers/channels are wired but
        /// unused — these tests never trigger refreshes or file events.
        async fn make_test_server(root: &Path) -> Arc<DaemonServer> {
            let (refresh_tx, _refresh_rx) = std::sync::mpsc::channel::<PathBuf>();
            let (config_reload_tx, _config_reload_rx) = std::sync::mpsc::channel::<PathBuf>();
            let (notify_tx, _notify_rx) =
                std::sync::mpsc::channel::<notify::Result<notify::Event>>();
            let watcher = RecommendedWatcher::new(notify_tx, Config::default()).unwrap();
            let server = Arc::new(DaemonServer::new(
                refresh_tx,
                Arc::new(DirtyRoots::new()),
                config_reload_tx,
                tokio::runtime::Handle::current(),
                watcher,
            ));

            // Open a real index for this root and register it directly without
            // running the full add_root flow (which triggers a refresh and
            // call-graph build we don't want here).
            let idx = crate::index::open(root).await.unwrap();
            let mut roots = server.roots.lock().unwrap();
            roots.insert(
                root.to_path_buf(),
                WatchedRoot {
                    last_refresh: Instant::now(),
                    refresh_window: std::collections::VecDeque::new(),
                    spin_flagged_until: None,
                    primed: true,
                    has_git_index: false,
                    index: Arc::new(std::sync::Mutex::new(idx)),
                    cached_rules: Arc::new(normalize_rules_config::RulesConfig::default()),
                    cached_walk: Arc::new(normalize_rules_config::WalkConfig::default()),
                    cached_scm_hashes: HashMap::new(),
                    serve_filter_pending: false,
                    context_index: Arc::new(Mutex::new(ContextIndex::new())),
                },
            );
            drop(roots);
            server
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn save_per_file_delta_first_call_lists_all_dirty_files() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;

            let syntax = vec![issue("a.rs", 1, "x"), issue("b.rs", 2, "y")];
            let delta = tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                move || server.save_per_file_diagnostics(&root, &syntax, &[], &[], "h")
            })
            .await
            .unwrap();

            let mut paths: Vec<&String> = delta.iter().map(|(p, _)| p).collect();
            paths.sort();
            assert_eq!(paths, vec!["a.rs", "b.rs"]);
            // No empty issue vec on first call.
            assert!(delta.iter().all(|(_, v)| !v.is_empty()));
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn save_per_file_delta_unchanged_call_is_empty() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;
            let syntax = vec![issue("a.rs", 1, "x")];

            let _first = tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                let s = syntax.clone();
                move || server.save_per_file_diagnostics(&root, &s, &[], &[], "h")
            })
            .await
            .unwrap();

            let second = tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                move || server.save_per_file_diagnostics(&root, &syntax, &[], &[], "h")
            })
            .await
            .unwrap();
            assert!(
                second.is_empty(),
                "unchanged content should produce no delta, got {:?}",
                second
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn save_per_file_delta_changed_file_only() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;
            let syntax_v1 = vec![issue("a.rs", 1, "x"), issue("b.rs", 2, "y")];

            let _first = tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                let s = syntax_v1.clone();
                move || server.save_per_file_diagnostics(&root, &s, &[], &[], "h")
            })
            .await
            .unwrap();

            // Only a.rs's issue text changes.
            let syntax_v2 = vec![issue("a.rs", 1, "DIFFERENT"), issue("b.rs", 2, "y")];
            let delta = tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                move || server.save_per_file_diagnostics(&root, &syntax_v2, &[], &[], "h")
            })
            .await
            .unwrap();

            assert_eq!(
                delta.len(),
                1,
                "delta should contain only a.rs: {:?}",
                delta
            );
            assert_eq!(delta[0].0, "a.rs");
            assert_eq!(delta[0].1.len(), 1);
            assert_eq!(delta[0].1[0].message, "DIFFERENT");
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn save_per_file_delta_clean_file_emits_empty_vec() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;
            let syntax_v1 = vec![issue("a.rs", 1, "x"), issue("b.rs", 2, "y")];

            let _first = tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                let s = syntax_v1.clone();
                move || server.save_per_file_diagnostics(&root, &s, &[], &[], "h")
            })
            .await
            .unwrap();

            // b.rs becomes clean (no longer present in the input).
            let syntax_v2 = vec![issue("a.rs", 1, "x")];
            let delta = tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                move || server.save_per_file_diagnostics(&root, &syntax_v2, &[], &[], "h")
            })
            .await
            .unwrap();

            let b_entry = delta
                .iter()
                .find(|(p, _)| p == "b.rs")
                .expect("b.rs should be in delta");
            assert!(
                b_entry.1.is_empty(),
                "deleted file must have empty Vec: {:?}",
                b_entry
            );
            // a.rs unchanged so must NOT be in the delta.
            assert!(
                delta.iter().all(|(p, _)| p != "a.rs"),
                "unchanged a.rs leaked into delta: {:?}",
                delta
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn write_json_mirror_produces_deterministic_keyed_output() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;

            let syntax = vec![issue("zzz.rs", 1, "z")];
            let fact = vec![issue("aaa.rs", 2, "a")];
            let native = vec![issue("mmm.rs", 3, "m")];

            tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                move || server.write_json_mirror(&root, &syntax, &fact, &native)
            })
            .await
            .unwrap();

            let path = dir.path().join(".normalize/diagnostics.json");
            assert!(path.exists());
            let body = std::fs::read_to_string(&path).unwrap();
            // Must be valid JSON.
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
            let obj = parsed.as_object().unwrap();
            // Only files-with-issues are present.
            let mut keys: Vec<&String> = obj.keys().collect();
            assert_eq!(
                keys.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                vec!["aaa.rs", "mmm.rs", "zzz.rs"],
                "BTreeMap key order must be deterministic / sorted"
            );
            keys.sort();
            // Tmp file should not survive an atomic rename.
            assert!(!dir.path().join(".normalize/diagnostics.json.tmp").exists());
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn write_json_mirror_omits_files_without_issues() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;
            tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                move || server.write_json_mirror(&root, &[], &[], &[])
            })
            .await
            .unwrap();
            let body =
                std::fs::read_to_string(dir.path().join(".normalize/diagnostics.json")).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(parsed.as_object().unwrap().len(), 0);
        }

        /// Regression: a non-empty delta returned by `save_per_file_diagnostics`
        /// must actually be broadcast on `event_tx`, otherwise no subscriber will
        /// ever receive `Event::DiagnosticsUpdated`.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn diagnostics_delta_is_broadcast_to_subscribers() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;
            let mut rx = server.event_tx.subscribe();

            let syntax = vec![issue("a.rs", 1, "x")];
            let server_clone = server.clone();
            let root = dir.path().to_path_buf();
            tokio::task::spawn_blocking(move || {
                let delta = server_clone.save_per_file_diagnostics(&root, &syntax, &[], &[], "h");
                server_clone.broadcast_diagnostics_delta(&root, delta);
            })
            .await
            .unwrap();

            let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
                .await
                .expect("timeout waiting for DiagnosticsUpdated")
                .expect("broadcast channel closed");
            match event {
                Event::DiagnosticsUpdated { updates, .. } => {
                    assert_eq!(updates.len(), 1);
                    assert_eq!(updates[0].0, "a.rs");
                }
                other => panic!("expected DiagnosticsUpdated, got {:?}", other),
            }
        }

        /// And the inverse: no broadcast for an empty delta (steady-state
        /// optimization) — otherwise every refresh would wake every subscriber
        /// even when nothing changed.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn empty_delta_not_broadcast() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;
            let mut rx = server.event_tx.subscribe();
            server.broadcast_diagnostics_delta(dir.path(), Vec::new());
            // No event should arrive within a short window.
            let res = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await;
            assert!(
                res.is_err(),
                "empty delta must not broadcast, got {:?}",
                res
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn save_per_file_writes_actually_persist_to_table() {
            let dir = tempfile::tempdir().unwrap();
            let server = make_test_server(dir.path()).await;
            let syntax = vec![issue("a.rs", 1, "x"), issue("a.rs", 2, "y")];
            tokio::task::spawn_blocking({
                let server = server.clone();
                let root = dir.path().to_path_buf();
                move || server.save_per_file_diagnostics(&root, &syntax, &[], &[], "h")
            })
            .await
            .unwrap();

            // Verify per-file table row count == files-with-issues count (=1).
            let idx_arc = server.get_root_index(dir.path()).unwrap();
            let (paths, blob) = tokio::task::spawn_blocking({
                let idx_arc = idx_arc.clone();
                let handle = tokio::runtime::Handle::current();
                move || {
                    let idx = idx_arc.lock().unwrap();
                    let paths = handle.block_on(idx.list_diagnostic_paths()).unwrap();
                    let blob = handle
                        .block_on(idx.load_diagnostics_for_file("a.rs", "h"))
                        .unwrap()
                        .unwrap();
                    (paths, blob)
                }
            })
            .await
            .unwrap();
            assert_eq!(paths, vec!["a.rs"]);
            let issues: Vec<Issue> =
                rkyv::from_bytes::<Vec<Issue>, rkyv::rancor::Error>(&blob).expect("rkyv decode");
            assert_eq!(issues.len(), 2);
        }

        // -- Tier 1 filter-at-serve --------------------------------------

        fn issue_with(file: &str, rule: &str, sev: Severity) -> Issue {
            Issue {
                file: file.to_string(),
                line: Some(1),
                column: Some(1),
                end_line: None,
                end_column: None,
                rule_id: rule.to_string(),
                message: "msg".to_string(),
                severity: sev,
                source: "test".to_string(),
                related: Vec::new(),
                suggestion: None,
            }
        }

        #[test]
        fn apply_filter_at_serve_drops_disabled_rules() {
            let mut issues = vec![
                issue_with("a.rs", "rule.a", Severity::Warning),
                issue_with("b.rs", "rule.b", Severity::Warning),
            ];
            let cfg = toml::from_str::<normalize_rules_config::RulesConfig>(
                r#"
[rule."rule.a"]
enabled = false
"#,
            )
            .unwrap();
            DaemonServer::apply_filter_at_serve(&mut issues, &cfg);
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].rule_id, "rule.b");
        }

        #[test]
        fn apply_filter_at_serve_overrides_severity() {
            let mut issues = vec![issue_with("a.rs", "rule.a", Severity::Warning)];
            let cfg = toml::from_str::<normalize_rules_config::RulesConfig>(
                r#"
[rule."rule.a"]
severity = "info"
"#,
            )
            .unwrap();
            DaemonServer::apply_filter_at_serve(&mut issues, &cfg);
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].severity, Severity::Info);
        }

        #[test]
        fn apply_filter_at_serve_drops_global_allowed_paths() {
            let mut issues = vec![
                issue_with("crates/foo/tests/it.rs", "rule.a", Severity::Warning),
                issue_with("crates/foo/src/lib.rs", "rule.a", Severity::Warning),
            ];
            let cfg = toml::from_str::<normalize_rules_config::RulesConfig>(
                r#"
global-allow = ["**/tests/**"]
"#,
            )
            .unwrap();
            DaemonServer::apply_filter_at_serve(&mut issues, &cfg);
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].file, "crates/foo/src/lib.rs");
        }

        #[test]
        fn apply_filter_at_serve_drops_per_rule_allowed_paths() {
            let mut issues = vec![
                issue_with("a.rs", "rule.a", Severity::Warning),
                issue_with("b.rs", "rule.a", Severity::Warning),
                issue_with("a.rs", "rule.b", Severity::Warning),
            ];
            let cfg = toml::from_str::<normalize_rules_config::RulesConfig>(
                r#"
[rule."rule.a"]
allow = ["a.rs"]
"#,
            )
            .unwrap();
            DaemonServer::apply_filter_at_serve(&mut issues, &cfg);
            // rule.a@a.rs dropped (allow); other two remain.
            assert_eq!(issues.len(), 2);
            assert!(
                issues
                    .iter()
                    .any(|i| i.rule_id == "rule.a" && i.file == "b.rs")
            );
            assert!(
                issues
                    .iter()
                    .any(|i| i.rule_id == "rule.b" && i.file == "a.rs")
            );
        }

        #[test]
        fn apply_filter_at_serve_no_config_is_noop() {
            let mut issues = vec![issue_with("a.rs", "rule.a", Severity::Warning)];
            let cfg = normalize_rules_config::RulesConfig::default();
            DaemonServer::apply_filter_at_serve(&mut issues, &cfg);
            assert_eq!(issues.len(), 1);
            assert_eq!(issues[0].severity, Severity::Warning);
        }
    }

    #[cfg(test)]
    mod spin_tests {
        use super::*;

        /// Build a minimal server with one registered root. Reuses a real
        /// SQLite-backed index (cheap; no files indexed) so the `roots` map has a
        /// valid `WatchedRoot` for the detector to mutate.
        async fn server_with_root(root: &Path) -> Arc<DaemonServer> {
            let (refresh_tx, _r) = std::sync::mpsc::channel::<PathBuf>();
            let (config_reload_tx, _c) = std::sync::mpsc::channel::<PathBuf>();
            let (notify_tx, _nx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
            let watcher = RecommendedWatcher::new(notify_tx, Config::default()).unwrap();
            let server = Arc::new(DaemonServer::new(
                refresh_tx,
                Arc::new(DirtyRoots::new()),
                config_reload_tx,
                tokio::runtime::Handle::current(),
                watcher,
            ));
            let idx = crate::index::open(root).await.unwrap();
            let mut roots = server.roots.lock().unwrap();
            roots.insert(
                root.to_path_buf(),
                WatchedRoot {
                    last_refresh: Instant::now(),
                    refresh_window: std::collections::VecDeque::new(),
                    spin_flagged_until: None,
                    primed: true,
                    has_git_index: false,
                    index: Arc::new(std::sync::Mutex::new(idx)),
                    cached_rules: Arc::new(normalize_rules_config::RulesConfig::default()),
                    cached_walk: Arc::new(normalize_rules_config::WalkConfig::default()),
                    cached_scm_hashes: HashMap::new(),
                    serve_filter_pending: false,
                    context_index: Arc::new(Mutex::new(ContextIndex::new())),
                },
            );
            drop(roots);
            server
        }

        fn flagged_until(server: &DaemonServer, root: &Path) -> Option<Instant> {
            let roots = server.roots.lock().unwrap();
            roots.get(root).and_then(|w| w.spin_flagged_until)
        }

        /// Self-write overlap + high density trips the flag and records a warning.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn overlap_plus_density_trips_spin() {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let server = server_with_root(root).await;

            // A changed set that includes the root's own state dir (index.sqlite).
            let state = crate::paths::get_normalize_dir(root);
            let changed = vec![state.join("index.sqlite"), root.join("src/lib.rs")];

            let now = Instant::now();
            // First SPIN_DENSITY_THRESHOLD-1 refreshes: density not yet met.
            for _ in 0..(SPIN_DENSITY_THRESHOLD - 1) {
                assert!(!server.record_refresh_and_detect_spin(root, &changed, now));
            }
            assert!(flagged_until(&server, root).is_none());
            // The threshold-th refresh trips it.
            assert!(server.record_refresh_and_detect_spin(root, &changed, now));
            assert!(flagged_until(&server, root).is_some());

            // A warning was recorded for status().
            let warnings = server.spin_warnings.lock().unwrap();
            assert_eq!(warnings.len(), 1);
            assert!(
                warnings[0]
                    .overlapping_paths
                    .iter()
                    .any(|p| p.contains("index.sqlite"))
            );
        }

        /// A busy repo (many source-file changes, none in the state dir) never
        /// trips, no matter the refresh density.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn busy_repo_does_not_trip_spin() {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let server = server_with_root(root).await;

            let changed: Vec<PathBuf> =
                (0..50).map(|i| root.join(format!("src/f{i}.rs"))).collect();
            let now = Instant::now();
            for _ in 0..(SPIN_DENSITY_THRESHOLD * 4) {
                assert!(!server.record_refresh_and_detect_spin(root, &changed, now));
            }
            assert!(flagged_until(&server, root).is_none());
            assert!(server.spin_warnings.lock().unwrap().is_empty());
        }

        /// Overlap without density (sparse self-writes) does not trip.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn overlap_without_density_does_not_trip() {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let server = server_with_root(root).await;
            let state = crate::paths::get_normalize_dir(root);
            let changed = vec![state.join("diagnostics.json")];

            // Each refresh is SPIN_WINDOW+1s apart, so the window never holds more
            // than one timestamp — density gate never met.
            let mut t = Instant::now();
            for _ in 0..(SPIN_DENSITY_THRESHOLD * 3) {
                assert!(!server.record_refresh_and_detect_spin(root, &changed, t));
                t += SPIN_WINDOW + Duration::from_secs(1);
            }
            assert!(flagged_until(&server, root).is_none());
        }

        /// Once flagged, the backoff window lifts after the cooldown elapses.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn backoff_lifts_after_cooldown() {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let server = server_with_root(root).await;
            let state = crate::paths::get_normalize_dir(root);
            let changed = vec![state.join("index.sqlite")];

            let now = Instant::now();
            for _ in 0..SPIN_DENSITY_THRESHOLD {
                server.record_refresh_and_detect_spin(root, &changed, now);
            }
            let until = flagged_until(&server, root).expect("should be flagged");

            // The cooldown endpoint is COOLDOWN past `now`; a check past it means
            // the dispatch loop would clear the flag and resume.
            assert!(until <= now + SPIN_COOLDOWN);
            let after = now + SPIN_COOLDOWN + Duration::from_secs(1);
            assert!(
                after >= until,
                "cooldown endpoint should be in the past now"
            );
        }

        /// Marking the same root dirty M times collapses to a single pending
        /// refresh — the coalescing property that bounds the backlog.
        #[test]
        fn dirty_roots_coalesces_same_root() {
            let dirty = DirtyRoots::new();
            let root = PathBuf::from("/repo/a");
            for _ in 0..1000 {
                dirty.mark(root.clone());
            }
            assert_eq!(dirty.pending_len(), 1, "1000 marks collapse to one entry");
            let drained = dirty.drain_blocking().expect("non-empty drain");
            assert_eq!(drained, vec![root]);
            assert_eq!(dirty.pending_len(), 0, "drain empties the set");
        }

        /// Distinct roots are all retained and all drained — coalescing is per-root,
        /// it never drops a root that needs refreshing.
        #[test]
        fn dirty_roots_keeps_distinct_roots() {
            let dirty = DirtyRoots::new();
            let roots: Vec<PathBuf> = (0..5)
                .map(|i| PathBuf::from(format!("/repo/{i}")))
                .collect();
            // Interleave marks so each root is dirtied multiple times.
            for _ in 0..3 {
                for r in &roots {
                    dirty.mark(r.clone());
                }
            }
            assert_eq!(dirty.pending_len(), 5, "5 distinct roots, deduped");
            let mut drained = dirty.drain_blocking().expect("non-empty drain");
            drained.sort();
            assert_eq!(drained, roots);
        }

        /// A consumer blocked on an empty queue wakes when a root is marked, and
        /// exits (returns `None`) when shutdown is requested with an empty queue.
        #[test]
        fn dirty_roots_blocks_then_wakes_and_shuts_down() {
            use std::sync::Arc as StdArc;
            let dirty = StdArc::new(DirtyRoots::new());

            // Consumer thread: first drain blocks until we mark a root.
            let consumer = {
                let dirty = dirty.clone();
                std::thread::spawn(move || {
                    let first = dirty.drain_blocking();
                    let second = dirty.drain_blocking();
                    (first, second)
                })
            };

            // Give the consumer a moment to block, then mark.
            std::thread::sleep(Duration::from_millis(50));
            dirty.mark(PathBuf::from("/repo/x"));
            // Then request shutdown so the second drain returns None.
            std::thread::sleep(Duration::from_millis(50));
            dirty.shutdown();

            let (first, second) = consumer.join().unwrap();
            assert_eq!(first, Some(vec![PathBuf::from("/repo/x")]));
            assert_eq!(second, None, "shutdown with empty queue returns None");
        }

        /// The `.git/index` spin signal: high refresh density combined with a
        /// non-draining coalescing queue trips the flag and records a warning.
        /// This is the signal the old detector missed — `.git/index` changes touch
        /// paths outside `.normalize/`, so the overlap-based detector never fired.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn native_refresh_density_with_queue_pressure_trips_spin() {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let server = server_with_root(root).await;

            // Simulate a non-draining queue: mark this root dirty so pending_len > 0.
            server.native_dirty.mark(root.to_path_buf());

            let now = Instant::now();
            for _ in 0..(SPIN_DENSITY_THRESHOLD - 1) {
                assert!(!server.record_native_refresh_and_detect_spin(root, now));
            }
            assert!(flagged_until(&server, root).is_none());
            // The threshold-th refresh, with the queue still non-empty, trips it.
            assert!(server.record_native_refresh_and_detect_spin(root, now));
            assert!(flagged_until(&server, root).is_some());

            let warnings = server.spin_warnings.lock().unwrap();
            assert_eq!(warnings.len(), 1);
            assert!(
                warnings[0]
                    .overlapping_paths
                    .iter()
                    .any(|p| p.contains(".git/index"))
            );
        }

        /// High native-refresh density with a *draining* queue (pending == 0) does
        /// not trip — a busy-but-healthy repo whose consumer keeps up is not a spin.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn native_refresh_density_without_queue_pressure_does_not_trip() {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let server = server_with_root(root).await;
            // Queue is empty (pending_len == 0) — the consumer kept up.
            let now = Instant::now();
            for _ in 0..(SPIN_DENSITY_THRESHOLD * 4) {
                assert!(!server.record_native_refresh_and_detect_spin(root, now));
            }
            assert!(flagged_until(&server, root).is_none());
            assert!(server.spin_warnings.lock().unwrap().is_empty());
        }

        /// The daemon log path follows the same dir convention as the socket and
        /// is named `daemon.log`. This verifies the chosen sink mechanism without
        /// mutating process env (which would race other tests in this binary).
        #[test]
        fn daemon_log_path_matches_socket_dir() {
            let log = daemon_log_path();
            let sock = global_socket_path();
            assert_eq!(log.parent(), sock.parent());
            assert_eq!(log.file_name().and_then(|n| n.to_str()), Some("daemon.log"));
        }
    }
}

#[cfg(unix)]
pub use unix_impl::{
    DaemonClient, daemon_log_path, global_socket_path, open_daemon_log_writer, run_daemon,
};

// ============================================================================
// Auto-start helper
// ============================================================================

/// Ensure daemon is running and watching this root.
pub fn maybe_start_daemon(root: &Path) {
    #[cfg(unix)]
    {
        use crate::config::NormalizeConfig;
        let config = NormalizeConfig::load(root);
        if !config.daemon.enabled() || !config.daemon.auto_start() || !config.index.enabled() {
            return;
        }

        // Skip auto-add for linked git worktrees. Each root holds its own
        // in-memory index, and worktrees of the same repo are near-duplicates —
        // auto-registering every `.claude/worktrees/agent-*` invocation caused
        // the daemon to balloon to many GB. Users can still explicitly register
        // a worktree with `normalize daemon add <path>` if they want.
        if is_git_worktree(root) {
            return;
        }

        let client = DaemonClient::new();
        if client.ensure_running() {
            // Add this root to the daemon
            let _ = client.add_root(root);
        }
    }

    #[cfg(not(unix))]
    let _ = root; // daemon not supported on non-Unix platforms
}

// ============================================================================
// Windows stubs
// ============================================================================

#[cfg(not(unix))]
pub fn global_socket_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("normalize")
        .join("daemon.sock")
}

#[cfg(not(unix))]
pub async fn run_daemon() -> Result<i32, Box<dyn std::error::Error>> {
    Err("normalize daemon is not supported on Windows".into())
}

#[cfg(not(unix))]
pub struct DaemonClient;

#[cfg(not(unix))]
impl DaemonClient {
    pub fn new() -> Self {
        Self
    }

    /// Stub for parity with the Unix impl. The Windows daemon is not
    /// supported, so the socket path is ignored.
    pub fn with_socket_path(_socket_path: PathBuf) -> Self {
        Self
    }

    pub fn is_available(&self) -> bool {
        false
    }

    pub fn ensure_running(&self) -> bool {
        false
    }

    pub fn send(&self, _request: &Request) -> Result<Response, String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn add_root(&self, _root: &Path) -> Result<Response, String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn remove_root(&self, _root: &Path) -> Result<Response, String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn list_roots(&self) -> Result<Response, String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn status(&self) -> Result<Response, String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn shutdown(&self) -> Result<(), String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn notify_files_changed(&self, _root: &Path, _paths: &[PathBuf]) -> Result<(), String> {
        Ok(()) // daemon not supported on Windows; silently no-op
    }

    pub fn run_rules(
        &self,
        _root: &Path,
        _filter_ids: Option<Vec<String>>,
        _filter_rule: Option<String>,
        _engine: Option<String>,
        _filter_files: Option<Vec<String>>,
    ) -> Result<Response, String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn query_context(
        &self,
        _root: &Path,
        _match_keys: Vec<(String, String)>,
        _all: bool,
        _dir_name: Option<String>,
    ) -> Result<Response, String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn watch_events(
        &self,
        _root: Option<&Path>,
        _on_event: impl FnMut(Event) -> bool,
    ) -> Result<(), String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }

    pub fn watch_events_binary(
        &self,
        _root: Option<&Path>,
        _on_event: impl FnMut(Event) -> bool,
    ) -> Result<(), String> {
        Err("normalize daemon is not supported on Windows".to_string())
    }
}

#[cfg(not(unix))]
impl Default for DaemonClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn is_git_worktree_detects_file_vs_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // No .git at all — not a worktree.
        assert!(!is_git_worktree(root));

        // .git is a directory (main repo) — not a worktree.
        let dot_git = root.join(".git");
        fs::create_dir(&dot_git).unwrap();
        assert!(!is_git_worktree(root));

        // .git is a file (linked worktree) — is a worktree.
        fs::remove_dir(&dot_git).unwrap();
        fs::write(&dot_git, "gitdir: /tmp/some/main/repo/.git/worktrees/x\n").unwrap();
        assert!(is_git_worktree(root));
    }
}
