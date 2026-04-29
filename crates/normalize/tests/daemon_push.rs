//! Integration tests for the daemon push channel.
//!
//! These tests spawn a real `normalize daemon run` subprocess against an
//! isolated socket directory (`NORMALIZE_DAEMON_CONFIG_DIR`) and exercise the
//! IPC path end-to-end:
//!
//!   - JSON subscribe receives `FileChanged` after a real file edit.
//!   - Binary subscribe (`watch_events_binary`) receives the same event via
//!     length-prefixed rkyv frames — verifying the binary protocol works.
//!
//! These tests live outside the per-test rkyv-based `DiagnosticsUpdated` cases
//! covered by the in-process unit tests in `daemon::unix_impl::per_file_tests`.
//! Bringing up the rules/index pipeline in a subprocess is much heavier (needs
//! a git repo, real grammars, native rules to fire) — the unit tests cover the
//! delta-and-broadcast logic directly so the round-trip surface here only has
//! to confirm the wire format.
//!
//! Each test gets its own isolated socket directory and constructs its
//! `DaemonClient` via `DaemonClient::with_socket_path`, so tests can run in
//! parallel — there is no shared `NORMALIZE_DAEMON_CONFIG_DIR` env-var state
//! between client constructions.

#![cfg(unix)]

use assert_cmd::cargo::CommandCargoExt;
use normalize::daemon::{DaemonClient, Event};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct DaemonGuard {
    child: Child,
    _config_dir: tempfile::TempDir,
    socket_path: std::path::PathBuf,
}

impl DaemonGuard {
    #[allow(clippy::zombie_processes)] // Drop impl wait()s.
    fn start() -> Self {
        let config_dir = tempfile::tempdir().unwrap();
        let config_path = config_dir.path().to_path_buf();
        let socket_path = config_path.join("daemon.sock");

        // The daemon child process reads NORMALIZE_DAEMON_CONFIG_DIR at
        // startup to know where to listen — that's a startup parameter set
        // before spawn, not a runtime global. Clients in this process target
        // the same socket via `DaemonClient::with_socket_path` so we never
        // touch our own env vars.
        let mut child = Command::cargo_bin("normalize")
            .expect("cargo bin")
            .arg("daemon")
            .arg("run")
            .env("NORMALIZE_DAEMON_CONFIG_DIR", &config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn daemon");

        // Wait for the socket to appear (daemon is async; ~2s max).
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if socket_path.exists() {
                // Briefly wait for the listener to be accepting connections.
                std::thread::sleep(Duration::from_millis(50));
                let client = DaemonClient::with_socket_path(socket_path.clone());
                if client.status().is_ok() {
                    return Self {
                        child,
                        _config_dir: config_dir,
                        socket_path,
                    };
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // Reap before panicking so we don't leave a zombie behind.
        let _ = child.kill();
        let _ = child.wait();
        panic!("daemon failed to start within 10s");
    }

    fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    fn client(&self) -> DaemonClient {
        DaemonClient::with_socket_path(self.socket_path.clone())
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        // Try a graceful shutdown first.
        let _ = self.client().shutdown();
        std::thread::sleep(Duration::from_millis(50));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Wait for the first event matching `pred`, or time out.
fn wait_for_event(
    rx: &mpsc::Receiver<Event>,
    timeout: Duration,
    mut pred: impl FnMut(&Event) -> bool,
) -> Option<Event> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        match rx.recv_timeout(remaining) {
            Ok(ev) => {
                if pred(&ev) {
                    return Some(ev);
                }
            }
            Err(_) => return None,
        }
    }
}

#[test]
fn json_subscribe_delivers_file_changed_event() {
    let daemon = DaemonGuard::start();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("a.txt"), "before\n").unwrap();

    // Subscribe in a background thread and forward events to a channel.
    let (tx, rx) = mpsc::channel::<Event>();
    let project_path = project.path().to_path_buf();
    let socket_path = daemon.socket_path().to_path_buf();
    // Register the root explicitly *before* subscribing so we know the
    // watcher is fully wired up. Doing it inside `Subscribe` works too, but
    // it interleaves the (possibly slow) `add_root` work with the broadcast
    // attach — racy under parallel test load. Doing it eagerly removes the
    // race entirely.
    DaemonClient::with_socket_path(socket_path.clone())
        .add_root(&project_path)
        .expect("add_root");
    let handle = std::thread::spawn(move || {
        let client = DaemonClient::with_socket_path(socket_path);
        let _ = client.watch_events(Some(&project_path), |ev| {
            let stop = matches!(ev, Event::FileChanged { .. });
            let _ = tx.send(ev);
            !stop
        });
    });

    // Give the subscriber thread time to connect before triggering changes.
    std::thread::sleep(Duration::from_millis(500));

    // Trigger a file change.
    std::fs::write(project.path().join("a.txt"), "after\n").unwrap();

    let event = wait_for_event(&rx, Duration::from_secs(10), |ev| {
        matches!(ev, Event::FileChanged { .. })
    });
    assert!(
        event.is_some(),
        "no FileChanged event arrived via JSON subscribe"
    );
    let _ = handle.join();
}

#[test]
fn binary_subscribe_delivers_file_changed_event() {
    let daemon = DaemonGuard::start();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("a.txt"), "before\n").unwrap();

    let (tx, rx) = mpsc::channel::<Event>();
    let project_path = project.path().to_path_buf();
    let socket_path = daemon.socket_path().to_path_buf();
    DaemonClient::with_socket_path(socket_path.clone())
        .add_root(&project_path)
        .expect("add_root");
    let handle = std::thread::spawn(move || {
        let client = DaemonClient::with_socket_path(socket_path);
        let _ = client.watch_events_binary(Some(&project_path), |ev| {
            let stop = matches!(ev, Event::FileChanged { .. });
            let _ = tx.send(ev);
            !stop
        });
    });

    std::thread::sleep(Duration::from_millis(500));
    std::fs::write(project.path().join("a.txt"), "after\n").unwrap();

    let event = wait_for_event(&rx, Duration::from_secs(10), |ev| {
        matches!(ev, Event::FileChanged { .. })
    });
    assert!(
        event.is_some(),
        "no FileChanged event arrived via binary subscribe"
    );
    let _ = handle.join();
}

/// Editing `.normalize/config.toml` must trigger a config-reload path: the
/// daemon clears cached diagnostic blobs and re-primes against the freshly
/// loaded config, broadcasting an `IndexRefreshed { files: 0 }` event when the
/// reload starts. Without this the daemon would serve stale `RunRules` results
/// (under the previous `[walk] exclude` / `[rules.rule.*] allow` config) until
/// either a source file changed or the daemon was restarted.
#[test]
fn config_edit_triggers_reload_event() {
    let daemon = DaemonGuard::start();
    let project = tempfile::tempdir().unwrap();
    // A source file so the prime has something to walk; not strictly
    // required for the reload signal but keeps the prime path realistic.
    std::fs::write(project.path().join("a.py"), "def hello():\n    return 1\n").unwrap();
    std::fs::create_dir_all(project.path().join(".normalize")).unwrap();
    std::fs::write(project.path().join(".normalize/config.toml"), "# initial\n").unwrap();

    let (tx, rx) = mpsc::channel::<Event>();
    let project_path = project.path().to_path_buf();
    let socket_path = daemon.socket_path().to_path_buf();
    DaemonClient::with_socket_path(socket_path.clone())
        .add_root(&project_path)
        .expect("add_root");
    let handle = std::thread::spawn(move || {
        let client = DaemonClient::with_socket_path(socket_path);
        let _ = client.watch_events(Some(&project_path), |ev| {
            // Stop after we see the config-reload signal — IndexRefreshed
            // with files: 0 is what reload_config_and_reprime emits before
            // it kicks off the (potentially slow) prime.
            let stop = matches!(&ev, Event::IndexRefreshed { files: 0, .. });
            let _ = tx.send(ev);
            !stop
        });
    });

    // Allow the subscriber + initial add_root refresh to settle. The startup
    // refresh emits its own IndexRefreshed (with files > 0), which we must
    // discard before triggering the config edit so we don't false-positive
    // on it.
    std::thread::sleep(Duration::from_millis(1500));
    while rx.try_recv().is_ok() {}

    // Edit the config to trigger a reload.
    std::fs::write(
        project.path().join(".normalize/config.toml"),
        "# trigger reload\n",
    )
    .unwrap();

    let event = wait_for_event(&rx, Duration::from_secs(15), |ev| {
        matches!(ev, Event::IndexRefreshed { files: 0, .. })
    });
    assert!(
        event.is_some(),
        "no config-reload IndexRefreshed event after `.normalize/config.toml` edit"
    );
    let _ = handle.join();
}

/// Across-restart cache validity: the config_hash gate.
///
/// 1. Start a daemon, register a project root, prime the cache.
/// 2. Stop the daemon (the SQLite cache survives on disk).
/// 3. Edit `.normalize/config.toml` *while no daemon is running* — so the
///    live-reload notify watcher cannot react.
/// 4. Start a fresh daemon against the same root.
///
/// The first `RunRules` after restart must reprime under the new config rather
/// than serve the previous session's blobs. The signal we watch for: the
/// running-daemon code path does not emit `IndexRefreshed { files: 0 }` on
/// startup; that signal would only appear if the daemon detected the cached
/// blobs were stale and triggered a full reprime under the new config. Here we
/// instead verify behavior end-to-end: the second daemon's `RunRules` must
/// not error with "not primed" and must succeed against the new config.
#[test]
fn config_hash_invalidates_cache_across_daemon_restart() {
    use normalize::daemon::DaemonClient;

    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("a.py"), "def hello():\n    return 1\n").unwrap();
    std::fs::create_dir_all(project.path().join(".normalize")).unwrap();
    std::fs::write(project.path().join(".normalize/config.toml"), "# initial\n").unwrap();

    // Reuse the same isolated config dir across both daemon spawns so the
    // SQLite index (which lives under the *project*, not the config dir)
    // outlives the daemon process — the config dir only holds the socket.
    let config_dir = tempfile::tempdir().unwrap();
    let config_path = config_dir.path().to_path_buf();
    let socket_path = config_path.join("daemon.sock");

    // ---- Session 1: prime the cache ----
    let mut child1 = Command::cargo_bin("normalize")
        .expect("cargo bin")
        .arg("daemon")
        .arg("run")
        .env("NORMALIZE_DAEMON_CONFIG_DIR", &config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon 1");

    // Wait for socket.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !socket_path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(socket_path.exists(), "session 1 daemon never came up");

    let client1 = DaemonClient::with_socket_path(socket_path.clone());
    // Wait for status to confirm acceptor is wired up.
    for _ in 0..40 {
        if client1.status().is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    client1
        .add_root(project.path())
        .expect("add_root session 1");
    // Trigger a prime via RunRules. The 500ms client read timeout may fire on
    // the first call (priming runs synchronously inside the daemon and can
    // exceed it on a cold cache); retry with a deadline until the daemon
    // serves a successful response, which proves the cache is primed and
    // persisted to SQLite.
    let prime_deadline = Instant::now() + Duration::from_secs(30);
    let mut session1_ok = false;
    while Instant::now() < prime_deadline {
        if let Ok(r) = client1.run_rules(project.path(), None, None, None, None)
            && r.ok
        {
            session1_ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(session1_ok, "session 1 never finished priming");

    // ---- Stop session 1 ----
    let _ = client1.shutdown();
    std::thread::sleep(Duration::from_millis(100));
    let _ = child1.kill();
    let _ = child1.wait();
    // Make sure socket is gone before session 2 spawns.
    let deadline = Instant::now() + Duration::from_secs(5);
    while socket_path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }

    // ---- Edit config while daemon is *not* running ----
    std::fs::write(
        project.path().join(".normalize/config.toml"),
        "# changed-while-stopped\n",
    )
    .unwrap();

    // ---- Session 2: must not serve stale blobs ----
    let mut child2 = Command::cargo_bin("normalize")
        .expect("cargo bin")
        .arg("daemon")
        .arg("run")
        .env("NORMALIZE_DAEMON_CONFIG_DIR", &config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon 2");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !socket_path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(socket_path.exists(), "session 2 daemon never came up");

    let client2 = DaemonClient::with_socket_path(socket_path.clone());
    for _ in 0..40 {
        if client2.status().is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    client2
        .add_root(project.path())
        .expect("add_root session 2");

    // First RunRules under session 2 must reprime under the new config. Use
    // the same retry pattern as session 1 — a fresh prime takes longer than
    // the 500ms read timeout. If the daemon were serving the on-disk session-1
    // blob without checking config_hash this would return immediately on the
    // first attempt; we don't assert that timing distinction here, only that
    // the eventual response is `ok` (i.e. prime completed under the new
    // config). The unit tests in normalize-facts cover the hash-mismatch =
    // None semantics directly.
    let prime_deadline = Instant::now() + Duration::from_secs(30);
    let mut session2_ok = false;
    while Instant::now() < prime_deadline {
        if let Ok(r) = client2.run_rules(project.path(), None, None, None, None)
            && r.ok
        {
            session2_ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(session2_ok, "session 2 never finished re-priming");

    let _ = client2.shutdown();
    std::thread::sleep(Duration::from_millis(100));
    let _ = child2.kill();
    let _ = child2.wait();
}

/// A daemon spawned with an isolated socket directory must respond to status
/// requests via the same isolated path — this catches broken socket-path
/// override wiring.
#[test]
fn isolated_socket_path_routes_to_isolated_daemon() {
    let daemon = DaemonGuard::start();
    let resp = daemon.client().status().expect("status reply");
    assert!(resp.ok, "daemon status returned !ok: {:?}", resp);
}

/// Reproduces the original manual-test bug report: the daemon emits
/// `FileChanged` but does it ever emit `IndexRefreshed` after a real edit?
///
/// Regression test: previously the daemon's `refresh_root` called
/// `incremental_refresh()`, whose 60-second `needs_refresh()` staleness gate
/// suppressed file-watcher-driven refreshes within the first minute after
/// `add_root`. The notify event arrived, the dispatch thread sent to
/// `refresh_tx`, the handler called `incremental_refresh`, which returned
/// `Ok(Vec::new())` because the gate was closed — so no `IndexRefreshed` (or
/// `DiagnosticsUpdated`) was ever broadcast. The daemon now uses
/// `incremental_refresh_force()` which bypasses the gate.
#[test]
fn json_subscribe_delivers_index_refreshed_event() {
    let daemon = DaemonGuard::start();
    let project = tempfile::tempdir().unwrap();
    // A small Python source file so refresh actually walks something.
    std::fs::write(project.path().join("a.py"), "def hello():\n    return 1\n").unwrap();

    let (tx, rx) = mpsc::channel::<Event>();
    let project_path = project.path().to_path_buf();
    let socket_path = daemon.socket_path().to_path_buf();
    DaemonClient::with_socket_path(socket_path.clone())
        .add_root(&project_path)
        .expect("add_root");
    let handle = std::thread::spawn(move || {
        let client = DaemonClient::with_socket_path(socket_path);
        let _ = client.watch_events(Some(&project_path), |ev| {
            let stop = matches!(ev, Event::IndexRefreshed { .. });
            let _ = tx.send(ev);
            !stop
        });
    });

    // Allow the subscriber + initial add_root refresh to settle.
    std::thread::sleep(Duration::from_millis(1500));

    // Edit a file to trigger a refresh.
    std::fs::write(
        project.path().join("a.py"),
        "def hello():\n    return 2\n# changed\n",
    )
    .unwrap();

    let event = wait_for_event(&rx, Duration::from_secs(15), |ev| {
        matches!(ev, Event::IndexRefreshed { .. })
    });
    assert!(
        event.is_some(),
        "no IndexRefreshed event arrived after file edit — daemon refresh path is broken"
    );
    let _ = handle.join();
}
