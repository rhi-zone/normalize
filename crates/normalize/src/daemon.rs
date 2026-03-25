//! Global daemon for watching multiple codebases and keeping indexes fresh.
//!
//! The daemon watches file changes across registered roots and incrementally
//! refreshes their indexes. Index queries go directly to SQLite files.
//!
//! The daemon uses Unix domain sockets for IPC and is only supported on Unix
//! platforms. On Windows all client functions return an unsupported error.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
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

    /// A watched root with its file watcher.
    struct WatchedRoot {
        #[allow(dead_code)] // Watcher must be kept alive
        watcher: RecommendedWatcher,
        last_refresh: Instant,
        /// Reverse-dependency graph: file → set of files that import it.
        /// Keys and values are absolute paths. Populated on root add, updated incrementally.
        rev_deps: HashMap<PathBuf, HashSet<PathBuf>>,
        /// Affected set from the most recent refresh: changed files ∪ their reverse dependents.
        /// Stored for future Datalog integration.
        last_affected: Vec<PathBuf>,
    }

    /// Global daemon server managing multiple roots.
    struct DaemonServer {
        roots: Mutex<HashMap<PathBuf, WatchedRoot>>,
        refresh_tx: Sender<PathBuf>,
        start_time: Instant,
        /// Broadcast channel for file-change and index-refresh events.
        /// Subscribers call `event_tx.subscribe()` to get a `broadcast::Receiver`.
        event_tx: broadcast::Sender<Event>,
    }

    impl DaemonServer {
        fn new(refresh_tx: Sender<PathBuf>) -> Self {
            // Capacity 1024: if a subscriber falls behind by more than 1024 events
            // it receives a RecvError::Lagged and we log the drop.
            let (event_tx, _) = broadcast::channel(1024);

            Self {
                roots: Mutex::new(HashMap::new()),
                refresh_tx,
                start_time: Instant::now(),
                event_tx,
            }
        }

        fn add_root(&self, root: PathBuf) -> Response {
            // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison = programmer error (thread panicked while holding lock)
            let mut roots = self.roots.lock().unwrap();

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

            // Initial index refresh and reverse-dep graph population
            // normalize-syntax-allow: rust/unwrap-in-impl - Runtime::new() only fails on OS resource exhaustion
            let rt = tokio::runtime::Runtime::new().unwrap();
            let mut rev_deps: HashMap<PathBuf, HashSet<PathBuf>> = HashMap::new();
            match rt.block_on(crate::index::open(&root)) {
                Ok(mut idx) => {
                    if let Err(e) = rt.block_on(idx.refresh()) {
                        return Response::err(&format!("Failed to index: {}", e));
                    }
                    if let Err(e) = rt.block_on(idx.incremental_call_graph_refresh()) {
                        eprintln!("Warning: call graph refresh failed: {}", e);
                    }
                    // Build reverse-dep graph from all resolved imports
                    match rt.block_on(idx.all_resolved_import_edges()) {
                        Ok(edges) => {
                            for (importer_rel, imported_rel) in edges {
                                let importer = root.join(&importer_rel);
                                let imported = root.join(&imported_rel);
                                rev_deps.entry(imported).or_default().insert(importer);
                            }
                        }
                        Err(e) => eprintln!(
                            "Warning: failed to build rev-dep graph for {:?}: {}",
                            root, e
                        ),
                    }
                }
                Err(e) => return Response::err(&format!("Failed to open index: {}", e)),
            }

            // Set up file watcher
            let tx = self.refresh_tx.clone();
            let event_tx = self.event_tx.clone();
            let root_clone = root.clone();
            let root_for_events = root.clone();
            let (notify_tx, notify_rx) = channel();

            let mut watcher = match RecommendedWatcher::new(notify_tx, Config::default()) {
                Ok(w) => w,
                Err(e) => return Response::err(&format!("Failed to create watcher: {}", e)),
            };

            if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
                return Response::err(&format!("Failed to watch: {}", e));
            }

            // Spawn thread to handle file events
            std::thread::spawn(move || {
                let debounce = Duration::from_millis(500);
                let mut last_event = Instant::now();

                for event in notify_rx.into_iter().flatten() {
                    // Skip .normalize directory
                    if event
                        .paths
                        .iter()
                        .all(|p| p.to_string_lossy().contains(".normalize"))
                    {
                        continue;
                    }

                    // Broadcast individual file-change events (un-debounced).
                    // SendError means no subscribers -- that is fine.
                    for path in &event.paths {
                        let _ = event_tx.send(Event::FileChanged {
                            path: path.clone(),
                            root: root_for_events.clone(),
                        });
                    }

                    if last_event.elapsed() >= debounce {
                        let _ = tx.send(root_clone.clone());
                        last_event = Instant::now();
                    }
                }
            });

            roots.insert(
                root.clone(),
                WatchedRoot {
                    watcher,
                    last_refresh: Instant::now(),
                    rev_deps,
                    last_affected: Vec::new(),
                },
            );

            Response::ok(serde_json::json!({"added": true, "root": root}))
        }

        fn remove_root(&self, root: &Path) -> Response {
            // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison = programmer error (thread panicked while holding lock)
            let mut roots = self.roots.lock().unwrap();
            if roots.remove(root).is_some() {
                Response::ok(serde_json::json!({"removed": true}))
            } else {
                Response::ok(serde_json::json!({"removed": false, "reason": "not watching"}))
            }
        }

        fn list_roots(&self) -> Response {
            // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison = programmer error (thread panicked while holding lock)
            let roots = self.roots.lock().unwrap();
            let list: Vec<&PathBuf> = roots.keys().collect();
            Response::ok(serde_json::json!(list))
        }

        fn status(&self) -> Response {
            // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison = programmer error (thread panicked while holding lock)
            let roots = self.roots.lock().unwrap();
            Response::ok(serde_json::json!({
                "uptime_secs": self.start_time.elapsed().as_secs(),
                "roots_watched": roots.len(),
                "pid": std::process::id(),
            }))
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
            }
        }

        fn refresh_root(&self, root: &Path) {
            // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison = programmer error (thread panicked while holding lock)
            let mut roots = self.roots.lock().unwrap();
            if let Some(watched) = roots.get_mut(root) {
                // normalize-syntax-allow: rust/unwrap-in-impl - Runtime::new() only fails on OS resource exhaustion
                let rt = tokio::runtime::Runtime::new().unwrap();
                match rt.block_on(crate::index::open(root)) {
                    Ok(mut idx) => {
                        match rt.block_on(idx.incremental_refresh()) {
                            Ok(changed) if !changed.is_empty() => {
                                if let Err(e) = rt.block_on(idx.incremental_call_graph_refresh()) {
                                    eprintln!("Call graph refresh error for {:?}: {}", root, e);
                                }

                                // Update reverse-dep graph for each changed file:
                                // remove its old outgoing edges, re-query, add new edges.
                                for abs_path in &changed {
                                    // Remove old forward-to-reverse edges for this file as importer
                                    let old_targets: Vec<PathBuf> = watched
                                        .rev_deps
                                        .iter()
                                        .filter_map(|(imported, importers)| {
                                            if importers.contains(abs_path) {
                                                Some(imported.clone())
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();
                                    for imported in &old_targets {
                                        if let Some(set) = watched.rev_deps.get_mut(imported) {
                                            set.remove(abs_path);
                                            if set.is_empty() {
                                                watched.rev_deps.remove(imported);
                                            }
                                        }
                                    }

                                    // Re-query and add new outgoing edges for this file
                                    if let Ok(rel) = abs_path.strip_prefix(root) {
                                        let rel_str = rel.to_string_lossy();
                                        if let Ok(new_targets) =
                                            rt.block_on(idx.resolved_imports_for_file(&rel_str))
                                        {
                                            for target_rel in new_targets {
                                                let target_abs = root.join(&target_rel);
                                                watched
                                                    .rev_deps
                                                    .entry(target_abs)
                                                    .or_default()
                                                    .insert(abs_path.clone());
                                            }
                                        }
                                    }
                                }

                                // Compute affected set: changed ∪ reverse dependents of changed
                                let changed_set: HashSet<&PathBuf> = changed.iter().collect();
                                let mut affected: HashSet<PathBuf> =
                                    changed.iter().cloned().collect();
                                for changed_file in &changed {
                                    if let Some(dependents) = watched.rev_deps.get(changed_file) {
                                        for dep in dependents {
                                            if !changed_set.contains(dep) {
                                                affected.insert(dep.clone());
                                            }
                                        }
                                    }
                                }

                                let affected_vec: Vec<PathBuf> = affected.into_iter().collect();
                                tracing::info!(
                                    changed = changed.len(),
                                    affected = affected_vec.len(),
                                    root = ?root,
                                    "index refreshed"
                                );
                                watched.last_affected = affected_vec.clone();

                                // Incremental Datalog evaluation: re-derive only the strata affected
                                // by the changed files.  The ENGINE_CACHE in normalize-rules is keyed
                                // by (root, rule_id) and lives for the lifetime of the process, so
                                // each watched root gets its own set of primed engines.  On the first
                                // call the engine is primed (full eval); subsequent calls retract
                                // stale facts and re-derive only the affected strata.
                                //
                                // The daemon does not broadcast individual fact diagnostics — that is
                                // the job of `normalize rules run` (CI path).  The daemon's role here
                                // is to prime the ENGINE_CACHE so that the next `normalize rules run`
                                // in this root can use the incremental path instead of a cold start.
                                {
                                    let config = NormalizeConfig::load(root);
                                    let diagnostics = rt.block_on(
                                        normalize_rules::collect_fact_diagnostics_incremental(
                                            root,
                                            &config.rules,
                                            None, // filter_ids — run all enabled rules
                                            None, // filter_rule — no single-rule filter
                                            Some(&watched.last_affected),
                                        ),
                                    );
                                    tracing::info!(
                                        root = ?root,
                                        diagnostics = diagnostics.len(),
                                        affected = watched.last_affected.len(),
                                        "incremental fact-rule eval complete"
                                    );
                                }

                                // Broadcast index-refresh event. SendError means no
                                // active subscribers -- that is fine.
                                let _ = self.event_tx.send(Event::IndexRefreshed {
                                    root: root.to_path_buf(),
                                    files: affected_vec.len(),
                                });
                            }
                            Err(e) => {
                                eprintln!("Refresh error for {:?}: {}", root, e);
                            }
                            _ => {}
                        }
                        watched.last_refresh = Instant::now();
                    }
                    Err(e) => eprintln!("Failed to open index for {:?}: {}", root, e),
                }
            }
        }
    }

    /// Run the global daemon server.
    #[tokio::main]
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

        // Channel for refresh requests from watchers
        let (refresh_tx, refresh_rx) = channel::<PathBuf>();

        let server = Arc::new(DaemonServer::new(refresh_tx));

        // Spawn refresh handler
        let server_refresh = server.clone();
        std::thread::spawn(move || {
            for root in refresh_rx {
                server_refresh.refresh_root(&root);
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
                let mut line = String::new();

                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    match serde_json::from_str::<Request>(&line) {
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
                    };

                    line.clear();
                }
            });
        }
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

            // Wait for socket to appear (daemon holds the flock, not us after this)
            for _ in 0..20 {
                if self.socket_path.exists() {
                    std::thread::sleep(Duration::from_millis(100));
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err("Daemon started but socket not created".to_string())
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
pub fn run_daemon() -> Result<i32, Box<dyn std::error::Error>> {
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
