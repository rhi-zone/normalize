//! Daemon management service for server-less CLI.

use crate::daemon::{self, DaemonClient, Event, global_socket_path};
use crate::output::OutputFormatter;
use server_less::cli;
use std::path::PathBuf;

/// Daemon management sub-service.
pub struct DaemonService;

/// Daemon status report returned by `normalize daemon status`.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonStatus {
    /// Whether the daemon process is currently running.
    pub running: bool,
    /// Path to the Unix socket the daemon listens on.
    pub socket: String,
    /// Process ID of the running daemon, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u64>,
    /// Number of seconds the daemon has been running, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_secs: Option<u64>,
    /// Number of project roots currently being watched, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots_watched: Option<u64>,
}

impl OutputFormatter for DaemonStatus {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        if !self.running {
            let _ = writeln!(out, "Daemon is not running");
            let _ = write!(out, "Socket: {}", self.socket);
            return out;
        }
        let _ = writeln!(out, "Daemon Status");
        let _ = writeln!(out, "  Running: yes");
        if let Some(pid) = self.pid {
            let _ = writeln!(out, "  PID: {}", pid);
        }
        if let Some(uptime) = self.uptime_secs {
            let _ = writeln!(out, "  Uptime: {} seconds", uptime);
        }
        if let Some(roots) = self.roots_watched {
            let _ = write!(out, "  Roots watched: {}", roots);
        }
        out
    }
}

/// Report for a daemon lifecycle action (`start` or `stop`).
///
/// Used when the caller triggers a daemon state change (starting or stopping the
/// background process). Distinct from `DaemonRootReport`, which covers root
/// management operations (add/remove watched directories).
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonActionReport {
    /// Whether the action completed successfully.
    pub success: bool,
    /// Optional human-readable description of the outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl OutputFormatter for DaemonActionReport {
    fn format_text(&self) -> String {
        if let Some(ref msg) = self.message {
            msg.clone()
        } else if self.success {
            "Done".to_string()
        } else {
            "Failed".to_string()
        }
    }
}

/// Report for a root management operation (`add` or `remove`).
///
/// Used when adding or removing a watched project root from the daemon. Distinct
/// from `DaemonActionReport`, which covers daemon lifecycle (start/stop).
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonRootReport {
    /// Whether the operation completed successfully.
    pub success: bool,
    /// Canonical path that was added or removed.
    pub path: String,
    /// Optional human-readable description of the outcome.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// True when the operation was simulated (--dry-run flag was passed).
    pub dry_run: bool,
}

impl OutputFormatter for DaemonRootReport {
    fn format_text(&self) -> String {
        if let Some(ref msg) = self.message {
            msg.clone()
        } else {
            self.path.clone()
        }
    }
}

/// Report for `normalize daemon run` (foreground daemon execution).
///
/// Returned when the daemon is run in the foreground (for debugging). Contains the
/// exit status message. Unlike `DaemonActionReport` (which covers start/stop lifecycle
/// commands) this type is specifically for the blocking foreground-run path.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonRunReport {
    /// Human-readable status message (e.g. "Daemon exited" or an error description).
    pub status: String,
}

impl OutputFormatter for DaemonRunReport {
    fn format_text(&self) -> String {
        self.status.clone()
    }
}

/// List of project roots currently watched by the daemon.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonRootsReport {
    /// Canonical paths of all watched project roots.
    pub roots: Vec<String>,
}

impl OutputFormatter for DaemonRootsReport {
    fn format_text(&self) -> String {
        if self.roots.is_empty() {
            "No roots being watched".to_string()
        } else {
            use std::fmt::Write as _;
            let mut out = String::new();
            let _ = writeln!(out, "Watched roots:");
            for root in &self.roots {
                let _ = writeln!(out, "  {}", root);
            }
            out
        }
    }
}

impl DaemonService {
    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

#[cli(name = "daemon", description = "Manage the global normalize daemon")]
impl DaemonService {
    /// Show daemon status
    ///
    /// Examples:
    ///   normalize daemon status              # check if daemon is running, show PID and uptime
    #[cli(display_with = "display_output")]
    pub fn status(&self) -> Result<DaemonStatus, String> {
        let client = DaemonClient::new();
        let socket = global_socket_path().display().to_string();

        if !client.is_available() {
            return Ok(DaemonStatus {
                running: false,
                socket,
                pid: None,
                uptime_secs: None,
                roots_watched: None,
            });
        }

        match client.status() {
            Ok(resp) if resp.ok => {
                let data = resp.data.unwrap_or_default();
                Ok(DaemonStatus {
                    running: true,
                    socket,
                    pid: data.get("pid").and_then(|v| v.as_u64()),
                    uptime_secs: data.get("uptime_secs").and_then(|v| v.as_u64()),
                    roots_watched: data.get("roots_watched").and_then(|v| v.as_u64()),
                })
            }
            Ok(resp) => Err(resp.error.unwrap_or_default()),
            Err(e) => Err(format!("Failed to get status: {}", e)),
        }
    }

    /// Stop the daemon
    ///
    /// Examples:
    ///   normalize daemon stop                # gracefully stop the running daemon
    #[cli(display_with = "display_output")]
    pub fn stop(&self) -> Result<DaemonActionReport, String> {
        let client = DaemonClient::new();

        if !client.is_available() {
            return Err("Daemon is not running".to_string());
        }

        match client.shutdown() {
            Ok(()) => Ok(DaemonActionReport {
                success: true,
                message: Some("Daemon stopped".to_string()),
            }),
            Err(e) => {
                // Connection reset is expected when daemon shuts down
                if e.contains("Connection reset") || e.contains("Broken pipe") {
                    Ok(DaemonActionReport {
                        success: true,
                        message: Some("Daemon stopped".to_string()),
                    })
                } else {
                    Err(format!("Failed to stop daemon: {}", e))
                }
            }
        }
    }

    /// Start the daemon (background)
    ///
    /// Examples:
    ///   normalize daemon start               # start the daemon in the background
    #[cli(display_with = "display_output")]
    pub fn start(&self) -> Result<DaemonActionReport, String> {
        let client = DaemonClient::new();

        if client.is_available() {
            return Err("Daemon is already running".to_string());
        }

        if client.ensure_running() {
            Ok(DaemonActionReport {
                success: true,
                message: Some("Daemon started".to_string()),
            })
        } else {
            Err("Failed to start daemon".to_string())
        }
    }

    /// Run the daemon in foreground (for debugging)
    ///
    /// Examples:
    ///   normalize daemon run                 # run daemon in foreground with log output
    #[cli(display_with = "display_output")]
    pub fn run(&self) -> Result<DaemonRunReport, String> {
        match daemon::run_daemon() {
            Ok(code) => {
                if code == 0 {
                    Ok(DaemonRunReport {
                        status: "Daemon exited".to_string(),
                    })
                } else {
                    Err(format!("Daemon exited with code {}", code))
                }
            }
            Err(e) => Err(format!("Daemon error: {}", e)),
        }
    }

    /// Add a root to watch
    ///
    /// Examples:
    ///   normalize daemon add                 # watch the current directory
    ///   normalize daemon add ~/projects/app  # watch a specific project root
    #[cli(display_with = "display_output")]
    pub fn add(
        &self,
        #[param(positional, help = "Path to the project root")] path: Option<String>,
        #[param(help = "Preview changes without applying")] dry_run: bool,
    ) -> Result<DaemonRootReport, String> {
        let path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let root = std::fs::canonicalize(&path).unwrap_or(path);

        if dry_run {
            return Ok(DaemonRootReport {
                success: true,
                path: root.display().to_string(),
                message: Some(format!("[dry-run] Would add: {}", root.display())),
                dry_run,
            });
        }

        let client = DaemonClient::new();

        if !client.ensure_running() {
            return Err("Failed to start daemon".to_string());
        }

        match client.add_root(&root) {
            Ok(resp) if resp.ok => {
                let added = resp
                    .data
                    .as_ref()
                    .and_then(|d| d.get("added"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let message = if added {
                    format!("Added: {}", root.display())
                } else {
                    let reason = resp
                        .data
                        .as_ref()
                        .and_then(|d| d.get("reason"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    format!("Already watching: {}", reason)
                };
                Ok(DaemonRootReport {
                    success: true,
                    path: root.display().to_string(),
                    message: Some(message),
                    dry_run,
                })
            }
            Ok(resp) => Err(resp.error.unwrap_or_default()),
            Err(e) => Err(format!("Failed: {}", e)),
        }
    }

    /// Remove a root from watching
    ///
    /// Examples:
    ///   normalize daemon remove              # stop watching the current directory
    ///   normalize daemon remove ~/projects/app  # stop watching a specific root
    #[cli(display_with = "display_output")]
    pub fn remove(
        &self,
        #[param(positional, help = "Path to the project root")] path: Option<String>,
        #[param(help = "Preview changes without applying")] dry_run: bool,
    ) -> Result<DaemonRootReport, String> {
        let path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let root = std::fs::canonicalize(&path).unwrap_or(path);

        if dry_run {
            return Ok(DaemonRootReport {
                success: true,
                path: root.display().to_string(),
                message: Some(format!("[dry-run] Would remove: {}", root.display())),
                dry_run,
            });
        }

        let client = DaemonClient::new();

        if !client.is_available() {
            return Err("Daemon is not running".to_string());
        }

        match client.remove_root(&root) {
            Ok(resp) if resp.ok => {
                let removed = resp
                    .data
                    .as_ref()
                    .and_then(|d| d.get("removed"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let message = if removed {
                    format!("Removed: {}", root.display())
                } else {
                    format!("Was not watching: {}", root.display())
                };
                Ok(DaemonRootReport {
                    success: true,
                    path: root.display().to_string(),
                    message: Some(message),
                    dry_run,
                })
            }
            Ok(resp) => Err(resp.error.unwrap_or_default()),
            Err(e) => Err(format!("Failed: {}", e)),
        }
    }

    /// Watch a root for file changes, streaming events to the terminal
    ///
    /// Starts the daemon if it is not running, ensures the root is watched,
    /// then streams events in real time until Ctrl-C. Output format:
    /// `[HH:MM:SS] modified src/main.rs`
    /// `[HH:MM:SS] index refreshed (3 files)`
    ///
    /// Examples:
    ///   normalize daemon watch               # watch current directory
    ///   normalize daemon watch ~/projects/app  # watch a specific root
    pub fn watch(
        &self,
        #[param(positional, help = "Path to the project root to watch")] path: Option<String>,
    ) -> Result<String, String> {
        let path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let root = std::fs::canonicalize(&path).unwrap_or(path);

        let client = DaemonClient::new();

        if !client.ensure_running() {
            return Err("Failed to start daemon".to_string());
        }

        eprintln!("Watching {} (press Ctrl-C to stop)", root.display());

        let result = client.watch_events(Some(&root), |event| {
            use std::time::SystemTime;
            use std::time::UNIX_EPOCH;

            // Format timestamp as [HH:MM:SS]
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let h = (secs / 3600) % 24;
            let m = (secs / 60) % 60;
            let s = secs % 60;
            let ts = format!("[{:02}:{:02}:{:02}]", h, m, s);

            match &event {
                Event::FileChanged { path, .. } => {
                    println!("{} modified {}", ts, path.display());
                }
                Event::IndexRefreshed { files, .. } => {
                    println!("{} index refreshed ({} files)", ts, files);
                }
            }

            true // continue streaming
        });

        match result {
            Ok(()) => Ok("Watch ended".to_string()),
            Err(e) if e.contains("Interrupted") || e.contains("EINTR") => {
                Ok("Watch ended".to_string())
            }
            Err(e) if e.contains("Connection reset") || e.contains("Broken pipe") => {
                Ok("Daemon disconnected".to_string())
            }
            Err(e) => Err(format!("Watch error: {}", e)),
        }
    }

    /// List all watched roots
    ///
    /// Examples:
    ///   normalize daemon list                # show all project roots being watched
    #[cli(display_with = "display_output")]
    pub fn list(&self) -> Result<DaemonRootsReport, String> {
        let client = DaemonClient::new();

        if !client.is_available() {
            return Err("Daemon is not running".to_string());
        }

        match client.list_roots() {
            Ok(resp) if resp.ok => {
                let roots = resp
                    .data
                    .as_ref()
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(DaemonRootsReport { roots })
            }
            Ok(resp) => Err(resp.error.unwrap_or_default()),
            Err(e) => Err(format!("Failed: {}", e)),
        }
    }
}
