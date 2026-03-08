//! Daemon management commands for normalize CLI.

use std::path::PathBuf;

/// Helper for default path
fn default_path() -> PathBuf {
    PathBuf::from(".")
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub enum DaemonAction {
    /// Show daemon status
    Status,

    /// Stop the daemon
    Stop,

    /// Start the daemon (background)
    Start,

    /// Run the daemon in foreground (for debugging)
    Run,

    /// Add a root to watch
    Add {
        /// Path to the project root
        #[serde(default = "default_path")]
        path: PathBuf,
    },

    /// Remove a root from watching
    Remove {
        /// Path to the project root
        #[serde(default = "default_path")]
        path: PathBuf,
    },

    /// List all watched roots
    List,
}
