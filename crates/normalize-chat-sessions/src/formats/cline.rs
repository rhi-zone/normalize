//! Cline session format parser.
//!
//! Cline stores one task per directory under `tasks/<taskId>/` with the
//! conversation as `api_conversation_history.json` (standard Anthropic
//! `MessageParam[]` format).
//!
//! Default storage roots:
//! - Linux:  `~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/`
//! - macOS:  `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/`
//! - Also:   `~/.cline/data/` (Cline home, cross-platform)

use super::{
    DiscoverError, ParseError, SessionLocation, SessionRef, SessionSource,
    anthropic_history::{discover_task_dirs, load_from_task_dir},
};
use crate::Session;
use std::path::{Path, PathBuf};

/// Cline session source (directory-per-task, `api_conversation_history.json`).
pub struct ClineFormat;

/// Cline VSCode extension identifier.
const CLINE_EXTENSION_ID: &str = "saoudrizwan.claude-dev";

fn cline_default_roots() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let home = PathBuf::from(home);

    let mut roots = vec![
        // Cross-platform Cline home
        home.join(".cline").join("data"),
    ];

    #[cfg(target_os = "macos")]
    roots.push(
        home.join("Library")
            .join("Application Support")
            .join("Code")
            .join("User")
            .join("globalStorage")
            .join(CLINE_EXTENSION_ID),
    );

    #[cfg(not(target_os = "macos"))]
    roots.push(
        home.join(".config")
            .join("Code")
            .join("User")
            .join("globalStorage")
            .join(CLINE_EXTENSION_ID),
    );

    roots
}

impl SessionSource for ClineFormat {
    fn name(&self) -> &'static str {
        "cline"
    }

    fn sessions_root(&self, _project: Option<&Path>) -> PathBuf {
        // Cline doesn't organise by project; return the first existing root.
        for root in cline_default_roots() {
            if root.exists() {
                return root;
            }
        }
        // Fallback: platform default even if it doesn't exist yet.
        cline_default_roots()
            .into_iter()
            .last()
            .unwrap_or_else(|| PathBuf::from("/tmp/.cline"))
    }

    fn default_roots(&self) -> Vec<PathBuf> {
        cline_default_roots()
    }

    fn detect(&self, path: &Path) -> f64 {
        if !path.is_dir() {
            return 0.0;
        }
        if !path.join("api_conversation_history.json").exists() {
            return 0.0;
        }
        let s = path.to_string_lossy();
        if s.contains(CLINE_EXTENSION_ID) || s.contains(".cline") {
            return 1.0;
        }
        // Generic directory-with-history — lower confidence (roo-code also matches)
        0.7
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
