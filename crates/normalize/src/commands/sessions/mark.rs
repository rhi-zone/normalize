/// Mark/unmark sessions as reviewed, stored in `.normalize/sessions-reviewed`.
use crate::output::OutputFormatter;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const REVIEWED_FILE: &str = "sessions-reviewed";

/// Resolve path to the reviewed-sessions file relative to a project root.
pub fn reviewed_path(root: Option<&Path>) -> PathBuf {
    let base = root.unwrap_or(Path::new("."));
    base.join(".normalize").join(REVIEWED_FILE)
}

/// Load the set of reviewed session IDs from `.normalize/sessions-reviewed`.
pub fn load_reviewed(root: Option<&Path>) -> HashSet<String> {
    let path = reviewed_path(root);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return HashSet::new();
    };
    content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Persist the set of reviewed session IDs to `.normalize/sessions-reviewed`.
fn save_reviewed(root: Option<&Path>, ids: &HashSet<String>) -> Result<(), String> {
    let path = reviewed_path(root);
    // Ensure .normalize/ exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }
    let mut lines: Vec<&str> = ids.iter().map(String::as_str).collect();
    lines.sort_unstable();
    let content = lines.join("\n") + if lines.is_empty() { "" } else { "\n" };
    std::fs::write(&path, content).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

/// Report returned by `sessions mark` and `sessions unmark`.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MarkReport {
    pub session_id: String,
    pub action: String,
    pub already: bool,
    /// True if this was a dry-run preview (nothing was written).
    #[serde(default)]
    pub dry_run: bool,
}

impl OutputFormatter for MarkReport {
    fn format_text(&self) -> String {
        let prefix = if self.dry_run { "[dry-run] " } else { "" };
        if self.already {
            format!(
                "{}Session {} was already {}d.",
                prefix, self.session_id, self.action
            )
        } else if self.dry_run {
            format!(
                "[dry-run] Would mark session {} as {}.",
                self.session_id, self.action
            )
        } else {
            format!("Session {} marked as {}.", self.session_id, self.action)
        }
    }
}

/// Mark a session as reviewed.
pub fn mark_session(
    session_id: &str,
    root: Option<&Path>,
    dry_run: bool,
) -> Result<MarkReport, String> {
    let mut ids = load_reviewed(root);
    let already = ids.contains(session_id);
    if !already && !dry_run {
        ids.insert(session_id.to_string());
        save_reviewed(root, &ids)?;
    }
    Ok(MarkReport {
        session_id: session_id.to_string(),
        action: "reviewed".to_string(),
        already,
        dry_run,
    })
}

/// Unmark a session (remove from reviewed list).
pub fn unmark_session(
    session_id: &str,
    root: Option<&Path>,
    dry_run: bool,
) -> Result<MarkReport, String> {
    let mut ids = load_reviewed(root);
    let already = !ids.contains(session_id);
    if !already && !dry_run {
        ids.remove(session_id);
        save_reviewed(root, &ids)?;
    }
    Ok(MarkReport {
        session_id: session_id.to_string(),
        action: "unreview".to_string(),
        already,
        dry_run,
    })
}
