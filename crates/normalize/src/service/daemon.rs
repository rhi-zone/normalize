//! Daemon management service for server-less CLI.

use crate::daemon::{self, DaemonClient, global_socket_path};
use server_less::cli;
use std::path::PathBuf;

/// Daemon management sub-service.
pub struct DaemonService;

/// Daemon status report.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonStatus {
    pub running: bool,
    pub socket: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots_watched: Option<u64>,
}

impl std::fmt::Display for DaemonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.running {
            writeln!(f, "Daemon is not running")?;
            write!(f, "Socket: {}", self.socket)?;
            return Ok(());
        }
        writeln!(f, "Daemon Status")?;
        writeln!(f, "  Running: yes")?;
        if let Some(pid) = self.pid {
            writeln!(f, "  PID: {}", pid)?;
        }
        if let Some(uptime) = self.uptime_secs {
            writeln!(f, "  Uptime: {} seconds", uptime)?;
        }
        if let Some(roots) = self.roots_watched {
            write!(f, "  Roots watched: {}", roots)?;
        }
        Ok(())
    }
}

/// Result of a daemon action (start/stop).
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonActionResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl std::fmt::Display for DaemonActionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref msg) = self.message {
            write!(f, "{}", msg)
        } else if self.success {
            write!(f, "Done")
        } else {
            write!(f, "Failed")
        }
    }
}

/// Result of add/remove root.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonRootResult {
    pub success: bool,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl std::fmt::Display for DaemonRootResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref msg) = self.message {
            write!(f, "{}", msg)
        } else {
            write!(f, "{}", self.path)
        }
    }
}

/// List of watched roots.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct DaemonRootList {
    pub roots: Vec<String>,
}

impl std::fmt::Display for DaemonRootList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.roots.is_empty() {
            write!(f, "No roots being watched")
        } else {
            writeln!(f, "Watched roots:")?;
            for root in &self.roots {
                writeln!(f, "  {}", root)?;
            }
            Ok(())
        }
    }
}

#[cli(name = "daemon", about = "Manage the global normalize daemon")]
impl DaemonService {
    /// Show daemon status
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
    pub fn stop(&self) -> Result<DaemonActionResult, String> {
        let client = DaemonClient::new();

        if !client.is_available() {
            return Err("Daemon is not running".to_string());
        }

        match client.shutdown() {
            Ok(()) => Ok(DaemonActionResult {
                success: true,
                message: Some("Daemon stopped".to_string()),
            }),
            Err(e) => {
                // Connection reset is expected when daemon shuts down
                if e.contains("Connection reset") || e.contains("Broken pipe") {
                    Ok(DaemonActionResult {
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
    pub fn start(&self) -> Result<DaemonActionResult, String> {
        let client = DaemonClient::new();

        if client.is_available() {
            return Err("Daemon is already running".to_string());
        }

        if client.ensure_running() {
            Ok(DaemonActionResult {
                success: true,
                message: Some("Daemon started".to_string()),
            })
        } else {
            Err("Failed to start daemon".to_string())
        }
    }

    /// Run the daemon in foreground (for debugging)
    pub fn run(&self) -> Result<String, String> {
        match daemon::run_daemon() {
            Ok(code) => {
                if code == 0 {
                    Ok("Daemon exited".to_string())
                } else {
                    Err(format!("Daemon exited with code {}", code))
                }
            }
            Err(e) => Err(format!("Daemon error: {}", e)),
        }
    }

    /// Add a root to watch
    pub fn add(
        &self,
        #[param(positional, help = "Path to the project root")] path: Option<String>,
    ) -> Result<DaemonRootResult, String> {
        let path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let root = std::fs::canonicalize(&path).unwrap_or(path);
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
                Ok(DaemonRootResult {
                    success: true,
                    path: root.display().to_string(),
                    message: Some(message),
                })
            }
            Ok(resp) => Err(resp.error.unwrap_or_default()),
            Err(e) => Err(format!("Failed: {}", e)),
        }
    }

    /// Remove a root from watching
    pub fn remove(
        &self,
        #[param(positional, help = "Path to the project root")] path: Option<String>,
    ) -> Result<DaemonRootResult, String> {
        let path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let root = std::fs::canonicalize(&path).unwrap_or(path);
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
                Ok(DaemonRootResult {
                    success: true,
                    path: root.display().to_string(),
                    message: Some(message),
                })
            }
            Ok(resp) => Err(resp.error.unwrap_or_default()),
            Err(e) => Err(format!("Failed: {}", e)),
        }
    }

    /// List all watched roots
    pub fn list(&self) -> Result<DaemonRootList, String> {
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
                Ok(DaemonRootList { roots })
            }
            Ok(resp) => Err(resp.error.unwrap_or_default()),
            Err(e) => Err(format!("Failed: {}", e)),
        }
    }
}
