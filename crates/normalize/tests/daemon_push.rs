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

#![cfg(unix)]

use assert_cmd::cargo::CommandCargoExt;
use normalize::daemon::{DaemonClient, Event};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct DaemonGuard {
    child: Child,
    _config_dir: tempfile::TempDir,
    config_dir_path: std::path::PathBuf,
}

impl DaemonGuard {
    #[allow(clippy::zombie_processes)] // Drop impl wait()s.
    fn start() -> Self {
        let config_dir = tempfile::tempdir().unwrap();
        let config_path = config_dir.path().to_path_buf();

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
        let socket = config_path.join("daemon.sock");
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if socket.exists() {
                // Briefly wait for the listener to be accepting connections.
                std::thread::sleep(Duration::from_millis(50));
                let client = with_env(&config_path, DaemonClient::new);
                if client.status().is_ok() {
                    return Self {
                        child,
                        _config_dir: config_dir,
                        config_dir_path: config_path,
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

    fn config_dir(&self) -> &std::path::Path {
        &self.config_dir_path
    }

    fn client(&self) -> DaemonClient {
        with_env(&self.config_dir_path, DaemonClient::new)
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

/// `DaemonClient::new()` reads the env var lazily inside `global_socket_path()`,
/// but the env var is process-global. Tests that spawn a daemon and use the
/// client must serialize via `#[serial_test::serial]` so the env var value
/// stays valid across the daemon spawn and the client construction.
fn with_env<R>(config_dir: &std::path::Path, f: impl FnOnce() -> R) -> R {
    // SAFETY: callers are serialized via `#[serial_test::serial]`, so no other
    // test in this crate is reading or writing this env var concurrently.
    unsafe {
        std::env::set_var("NORMALIZE_DAEMON_CONFIG_DIR", config_dir);
    }
    f()
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
#[serial_test::serial]
fn json_subscribe_delivers_file_changed_event() {
    let daemon = DaemonGuard::start();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("a.txt"), "before\n").unwrap();

    // Subscribe in a background thread and forward events to a channel.
    let (tx, rx) = mpsc::channel::<Event>();
    let project_path = project.path().to_path_buf();
    let config_path = daemon.config_dir().to_path_buf();
    let handle = std::thread::spawn(move || {
        let client = with_env(&config_path, DaemonClient::new);
        // Subscribe + auto-add the root.
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
#[serial_test::serial]
fn binary_subscribe_delivers_file_changed_event() {
    let daemon = DaemonGuard::start();
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("a.txt"), "before\n").unwrap();

    let (tx, rx) = mpsc::channel::<Event>();
    let project_path = project.path().to_path_buf();
    let config_path = daemon.config_dir().to_path_buf();
    let handle = std::thread::spawn(move || {
        let client = with_env(&config_path, DaemonClient::new);
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

/// A daemon spawned with an isolated socket directory must respond to status
/// requests via the same isolated path — this catches broken socket-path
/// override wiring.
#[test]
#[serial_test::serial]
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
#[serial_test::serial]
fn json_subscribe_delivers_index_refreshed_event() {
    let daemon = DaemonGuard::start();
    let project = tempfile::tempdir().unwrap();
    // A small Python source file so refresh actually walks something.
    std::fs::write(project.path().join("a.py"), "def hello():\n    return 1\n").unwrap();

    let (tx, rx) = mpsc::channel::<Event>();
    let project_path = project.path().to_path_buf();
    let config_path = daemon.config_dir().to_path_buf();
    let handle = std::thread::spawn(move || {
        let client = with_env(&config_path, DaemonClient::new);
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
