//! Roo-Code session format parser.
//!
//! Roo-Code stores one task per directory under `tasks/<taskId>/` with the
//! conversation as `api_conversation_history.json`. The format extends the
//! standard Anthropic `MessageParam` with extra fields (`ts`, `isSummary`,
//! `reasoning_content`, `condenseId`, `truncationId`, etc.).
//!
//! Default storage roots:
//! - Linux:  `~/.config/Code/User/globalStorage/rooveterinaryinc.roo-cline/`
//! - macOS:  `~/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/`

use super::{
    DiscoverError, ParseError, SessionLocation, SessionRef, SessionSource,
    anthropic_history::{discover_task_dirs, load_from_task_dir},
};
use crate::Session;
use std::path::{Path, PathBuf};

/// Roo-Code session source (directory-per-task, extended `api_conversation_history.json`).
pub struct RooCodeFormat;

/// Roo-Code VSCode extension identifier.
const ROO_EXTENSION_ID: &str = "rooveterinaryinc.roo-cline";

fn roo_default_roots() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let home = PathBuf::from(home);

    #[cfg(target_os = "macos")]
    return vec![
        home.join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
            .join("globalStorage")
            .join(ROO_EXTENSION_ID),
    ];

    #[cfg(not(target_os = "macos"))]
    vec![
        home.join(".config")
            .join("Code")
            .join("User")
            .join("globalStorage")
            .join(ROO_EXTENSION_ID),
    ]
}

impl SessionSource for RooCodeFormat {
    fn name(&self) -> &'static str {
        "roo-code"
    }

    fn sessions_root(&self, _project: Option<&Path>) -> PathBuf {
        for root in roo_default_roots() {
            if root.exists() {
                return root;
            }
        }
        roo_default_roots()
            .into_iter()
            .last()
            .unwrap_or_else(|| PathBuf::from("/tmp/.roo-code"))
    }

    fn default_roots(&self) -> Vec<PathBuf> {
        roo_default_roots()
    }

    fn detect(&self, path: &Path) -> f64 {
        if !path.is_dir() {
            return 0.0;
        }
        if !path.join("api_conversation_history.json").exists() {
            return 0.0;
        }
        let s = path.to_string_lossy();
        if s.contains(ROO_EXTENSION_ID) || s.contains("roo-cline") || s.contains("roo-code") {
            return 1.0;
        }
        // Generic directory-with-history — lower confidence (cline also matches)
        0.65
    }

    fn discover(&self, root: &Path) -> Result<Vec<SessionRef>, DiscoverError> {
        discover_task_dirs(root, self.name())
    }

    fn load(&self, r: &SessionRef) -> Result<Session, ParseError> {
        let task_dir = match &r.location {
            SessionLocation::Directory(p) => p.as_path(),
            _ => &r.path,
        };
        let task_id = task_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        load_from_task_dir(task_dir, self.name(), task_id)
    }
}
