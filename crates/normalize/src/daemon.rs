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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// A file was modified, created, or deleted.
    FileChanged {
        /// Canonical path of the file that changed.
        path: PathBuf,
        /// The watched root that owns this file.
        root: PathBuf,
    },
    /// The index was refreshed after file changes.
    IndexRefreshed {
        /// The root whose index was refreshed.
        root: PathBuf,
        /// Number of files reindexed.
        files: usize,
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
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    use tokio::net::UnixListener;
    use tokio::sync::broadcast;

    /// Get the daemon lock file path (~/.config/normalize/daemon.lock)
    /// Used by the daemon process to ensure only one instance runs.
    fn daemon_lock_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("normalize")
            .join("daemon.lock")
    }

    /// Get the spawn lock file path (~/.config/normalize/daemon-spawn.lock)
    /// Used by clients to serialize daemon spawn attempts.
    fn spawn_lock_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("normalize")
            .join("daemon-spawn.lock")
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

    /// Get global daemon socket path (~/.config/normalize/daemon.sock)
    pub fn global_socket_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("normalize")
            .join("daemon.sock")
    }

    /// A watched root. The shared watcher is owned by `DaemonServer`; this struct
    /// tracks per-root metadata only.
    struct WatchedRoot {
        last_refresh: Instant,
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
    }

    /// Global daemon server managing multiple roots.
    struct DaemonServer {
        roots: Mutex<HashMap<PathBuf, WatchedRoot>>,
        refresh_tx: Sender<PathBuf>,
        /// Sender for native-rules-only refresh requests (triggered by .git/index changes).
        native_refresh_tx: Sender<PathBuf>,
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
            native_refresh_tx: Sender<PathBuf>,
            runtime_handle: tokio::runtime::Handle,
            watcher: RecommendedWatcher,
        ) -> Self {
            // Capacity 1024: if a subscriber falls behind by more than 1024 events
            // it receives a RecvError::Lagged and we log the drop.
            let (event_tx, _) = broadcast::channel(1024);

            Self {
                roots: Mutex::new(HashMap::new()),
                refresh_tx,
                native_refresh_tx,
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

            // Register the root with the shared watcher
            let git_index_path = root.join(".git").join("index");
            let has_git_index = git_index_path.exists();
            {
                let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
                    return Response::err(&format!("Failed to watch: {}", e));
                }
                // Also watch .git/index if it exists so `git add` triggers a native-rules refresh.
                if has_git_index
                    && let Err(e) = watcher.watch(&git_index_path, RecursiveMode::NonRecursive)
                {
                    eprintln!("Warning: failed to watch .git/index for {:?}: {}", root, e);
                }
            }

            roots.insert(
                root.clone(),
                WatchedRoot {
                    last_refresh: Instant::now(),
                    primed: false,
                    has_git_index,
                    index,
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
            Response::ok(serde_json::json!({
                "uptime_secs": self.start_time.elapsed().as_secs(),
                "roots_watched": roots.len(),
                "pid": std::process::id(),
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
            }
        }

        fn run_rules(
            &self,
            root: PathBuf,
            filter_ids: Option<Vec<String>>,
            filter_rule: Option<String>,
            engine: Option<String>,
            filter_files: Option<Vec<String>>,
        ) -> Response {
            let needs_prime = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                match roots.get(&root) {
                    Some(w) => !w.primed,
                    None => return Response::err("root not watched"),
                }
            };

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
                    self.runtime_handle.block_on(idx.load_diagnostics_blob(eng))
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
            let needs_prime = {
                let roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
                match roots.get(&root) {
                    Some(w) => !w.primed,
                    None => return RawResponse::Error("root not watched".into()),
                }
            };

            if needs_prime {
                self.prime_diagnostics_cache(&root);
            }

            let idx_arc = match self.get_root_index(&root) {
                Some(a) => a,
                None => return RawResponse::Error("root not watched".into()),
            };

            // Fast path A: no filter and no engine filter → serve "all" blob directly.
            if filter_ids.is_none()
                && filter_rule.is_none()
                && engine.is_none()
                && filter_files.is_none()
            {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                match tokio::task::block_in_place(|| {
                    self.runtime_handle
                        .block_on(idx.load_diagnostics_blob("all"))
                }) {
                    Ok(Some(blob)) => return RawResponse::Frame(blob),
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
                        .block_on(idx.load_diagnostics_for_files(files))
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
                    self.runtime_handle.block_on(idx.load_diagnostics_blob(eng))
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

        /// Prime the diagnostics cache for a root with a full evaluation of all engines.
        /// Results are persisted to the SQLite index and immediately dropped from heap.
        /// Called lazily on the first `RunRules` request or after config invalidation.
        fn prime_diagnostics_cache(&self, root: &Path) {
            let config = NormalizeConfig::load(root);

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
            self.save_diagnostics_to_index(root, "syntax", &syntax_issues);
            self.save_diagnostics_to_index(root, "fact", &fact_issues);
            self.save_diagnostics_to_index(root, "native", &native_issues);
            // Build "all" combined blob for fast no-filter hot path.
            self.save_all_blob(root, &syntax_issues, &fact_issues, &native_issues);
            // Per-file table + JSON mirror for ephemeral consumers.
            self.save_per_file_diagnostics(root, &syntax_issues, &fact_issues, &native_issues);
            self.write_json_mirror(root, &syntax_issues, &fact_issues, &native_issues);
            drop(syntax_issues);
            drop(fact_issues);
            drop(native_issues);

            let mut roots = self.roots.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(watched) = roots.get_mut(root) {
                watched.primed = true;
            }
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
                    .block_on(idx.save_diagnostics_blob(engine, &blob))
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
        ) {
            let mut all: Vec<normalize_output::diagnostics::Issue> =
                Vec::with_capacity(syntax.len() + fact.len() + native.len());
            all.extend_from_slice(syntax);
            all.extend_from_slice(fact);
            all.extend_from_slice(native);
            self.save_diagnostics_to_index(root, "all", &all);
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
            let mut all: Vec<normalize_output::diagnostics::Issue> = Vec::new();
            for eng in &["syntax", "fact", "native"] {
                let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());
                if let Ok(Some(blob)) = tokio::task::block_in_place(|| {
                    self.runtime_handle.block_on(idx.load_diagnostics_blob(eng))
                }) && let Ok(issues) = rkyv::from_bytes::<
                    Vec<normalize_output::diagnostics::Issue>,
                    rkyv::rancor::Error,
                >(&blob)
                {
                    all.extend(issues);
                }
            }
            self.save_diagnostics_to_index(root, "all", &all);
        }

        /// Group all current issues by path and reconcile per-file storage.
        /// Called after every prime or incremental refresh so the per-file
        /// table reflects the same state as the per-engine "all" blob.
        fn save_per_file_diagnostics(
            &self,
            root: &Path,
            syntax: &[normalize_output::diagnostics::Issue],
            fact: &[normalize_output::diagnostics::Issue],
            native: &[normalize_output::diagnostics::Issue],
        ) {
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
                return;
            };
            let idx = idx_arc.lock().unwrap_or_else(|e| e.into_inner());

            // Files currently in the table that no longer have issues -> delete.
            let existing_paths: Vec<String> = match tokio::task::block_in_place(|| {
                self.runtime_handle.block_on(idx.list_diagnostic_paths())
            }) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("failed to list diagnostic paths: {}", e);
                    return;
                }
            };
            let new_paths: HashSet<&String> = by_path.keys().collect();
            let deletes: Vec<String> = existing_paths
                .into_iter()
                .filter(|p| !new_paths.contains(p))
                .collect();

            // Serialize each file's issues to rkyv.
            let mut upserts: Vec<(String, Vec<u8>)> = Vec::with_capacity(by_path.len());
            for (path, issues) in by_path {
                match rkyv::to_bytes::<rkyv::rancor::Error>(&issues) {
                    Ok(b) => upserts.push((path, b.to_vec())),
                    Err(e) => tracing::warn!("rkyv serialize per-file diagnostics: {}", e),
                }
            }

            if let Err(e) = tokio::task::block_in_place(|| {
                self.runtime_handle
                    .block_on(idx.save_diagnostics_per_file(&upserts, &deletes))
            }) {
                tracing::warn!("failed to save per-file diagnostics: {}", e);
            }
        }

        /// Rebuild the per-file diagnostics table by reading the three per-engine
        /// blobs from SQLite. Used when only one engine has been refreshed
        /// (e.g. `refresh_native_rules`).
        fn rebuild_per_file_diagnostics(&self, root: &Path) {
            let Some(idx_arc) = self.get_root_index(root) else {
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
                    self.runtime_handle.block_on(idx.load_diagnostics_blob(eng))
                }) && let Ok(issues) = rkyv::from_bytes::<
                    Vec<normalize_output::diagnostics::Issue>,
                    rkyv::rancor::Error,
                >(&blob)
                {
                    *sink = issues;
                }
            }
            self.save_per_file_diagnostics(root, &syntax, &fact, &native);
            self.write_json_mirror(root, &syntax, &fact, &native);
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

            let (
                missing_res,
                summary_res,
                stale_res,
                examples_res,
                refs_res,
                ratchet_res,
                budget_res,
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

            normalize_rules::apply_native_rules_config(&mut report, rules_config);
            report.issues
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
                match self.runtime_handle.block_on(idx.incremental_refresh()) {
                    Ok(changed) if !changed.is_empty() => {
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
                                .block_on(idx.load_diagnostics_blob("syntax"))
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
                        self.save_diagnostics_to_index(root, "syntax", &syntax_issues);
                        self.save_diagnostics_to_index(root, "fact", &fact_issues);
                        self.save_diagnostics_to_index(root, "native", &native_issues);
                        // Build "all" combined blob for fast no-filter hot path.
                        self.save_all_blob(root, &syntax_issues, &fact_issues, &native_issues);
                        // Per-file table + JSON mirror for ephemeral consumers.
                        self.save_per_file_diagnostics(
                            root,
                            &syntax_issues,
                            &fact_issues,
                            &native_issues,
                        );
                        self.write_json_mirror(root, &syntax_issues, &fact_issues, &native_issues);
                        drop(syntax_issues);
                        drop(fact_issues);
                        drop(native_issues);
                        drop(affected_vec);

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
                            root: root.to_path_buf(),
                            files: changed.len(),
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

            // Persist to SQLite and drop immediately.
            self.save_diagnostics_to_index(root, "native", &new_native_issues);
            drop(new_native_issues);
            // Rebuild "all" blob from existing per-engine blobs.
            self.rebuild_all_blob(root);
            // Rebuild per-file table + JSON mirror from per-engine blobs.
            self.rebuild_per_file_diagnostics(root);
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
        // Channel for native-rules-only refresh requests triggered by .git/index changes
        let (native_refresh_tx, native_refresh_rx) = channel::<PathBuf>();

        // Create shared file watcher that handles events for all roots
        let (notify_tx, notify_rx) = channel::<notify::Result<notify::Event>>();
        let shared_watcher = RecommendedWatcher::new(notify_tx, Config::default())
            .expect("failed to create shared file watcher");

        let server = Arc::new(DaemonServer::new(
            refresh_tx,
            native_refresh_tx,
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
            let mut last_refresh: HashMap<PathBuf, Instant> = HashMap::new();
            let mut last_native: HashMap<PathBuf, Instant> = HashMap::new();

            for result in notify_rx {
                let event = match result {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("notify error: {}", e);
                        continue;
                    }
                };

                // Skip .normalize directory changes
                if event
                    .paths
                    .iter()
                    .all(|p| p.to_string_lossy().contains(".normalize"))
                {
                    continue;
                }

                // Collect dispatch targets under a brief lock, then release before sending
                let mut to_event: Vec<(PathBuf, PathBuf)> = Vec::new(); // (path, root)
                let mut to_refresh: Vec<PathBuf> = Vec::new();
                let mut to_native: Vec<PathBuf> = Vec::new();

                {
                    let roots = server_dispatch
                        .roots
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    for path in &event.paths {
                        // Find the root this path belongs to
                        let root = roots.keys().find(|r| path.starts_with(r.as_path()));
                        if let Some(root) = root {
                            to_event.push((path.clone(), root.clone()));

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
                    }
                } // lock released before sending

                // Broadcast un-debounced file-change events
                for (path, root) in to_event {
                    let _ = server_dispatch
                        .event_tx
                        .send(Event::FileChanged { path, root });
                }
                for root in to_refresh {
                    let _ = server_dispatch.refresh_tx.send(root);
                }
                for root in to_native {
                    let _ = server_dispatch.native_refresh_tx.send(root);
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

        // Spawn native-rules-only refresh handler (for .git/index changes)
        let server_native = server.clone();
        std::thread::spawn(move || {
            for root in native_refresh_rx {
                server_native.refresh_native_rules(&root);
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
            Ok(_) => RawResponse::Error("rkyv mode only supports run_rules".into()),
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
        pub fn new() -> Self {
            Self {
                socket_path: global_socket_path(),
            }
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

            Command::new(&current_exe)
                .arg("daemon")
                .arg("run")
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
    }

    impl Default for DaemonClient {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(unix)]
pub use unix_impl::{DaemonClient, global_socket_path, run_daemon};

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

    pub fn watch_events(
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
