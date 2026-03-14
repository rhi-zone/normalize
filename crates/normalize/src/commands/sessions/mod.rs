//! Sessions command - analyze Claude Code and other agent session logs.

pub mod analyze;
pub mod grep;
pub mod list;
pub mod messages;
pub mod plans;
#[cfg(feature = "sessions-web")]
mod serve;
pub mod show;
pub mod stats;

pub use grep::{SessionGrepReport, build_grep_report};
pub use list::{SessionListReport, build_session_list};
pub use messages::{MessagesReport, build_messages_report};
pub use plans::{PlanContent, PlansListReport, build_plan_content, build_plans_list};
#[cfg(feature = "sessions-web")]
pub use serve::serve_sessions;
pub use show::{SessionShowReport, build_analyze_report, build_show_report};
pub use stats::{build_stats_data, show_stats_grouped};

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::sessions::{FormatRegistry, LogFormat};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

/// Format an age in seconds to a human-readable string.
pub(crate) fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Resolve a session identifier to one or more file paths.
/// Tries literal (exact/prefix) matching first, then falls back to fuzzy matching.
pub(crate) fn resolve_session_paths(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Vec<PathBuf> {
    let literal = resolve_session_paths_literal(session_id, project, format_name);
    if !literal.is_empty() {
        return literal;
    }

    // Fuzzy fallback
    if let Some((path, matched_stem)) = resolve_session_fuzzy(session_id, project, format_name) {
        eprintln!("fuzzy match: {}", matched_stem);
        return vec![path];
    }

    Vec::new()
}

/// Resolve a session identifier using exact match or prefix only.
/// Supports: full path, session ID prefix, glob pattern.
pub(crate) fn resolve_session_paths_literal(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Vec<PathBuf> {
    let session_path = Path::new(session_id);

    // If it's a full path, use it directly
    if session_path.is_file() {
        return vec![session_path.to_path_buf()];
    }

    // If it looks like a glob pattern, expand it
    if (session_id.contains('*') || session_id.contains('?'))
        && let Ok(entries) = glob::glob(session_id)
    {
        let paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|p| p.is_file())
            .collect();
        if !paths.is_empty() {
            return paths;
        }
    }

    // Otherwise, try to find it as a session ID in the format's directory
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => match registry.get(name) {
            Some(f) => f,
            None => return Vec::new(),
        },
        None => match registry.get("claude") {
            Some(f) => f,
            None => return Vec::new(),
        },
    };

    let sessions = format.list_sessions(project);

    // Match by session ID prefix (file stem)
    for s in &sessions {
        if let Some(stem) = s.path.file_stem().and_then(|s| s.to_str())
            && (stem == session_id || stem.starts_with(session_id))
        {
            return vec![s.path.clone()];
        }
    }

    // No match
    Vec::new()
}

/// Fuzzy-match a session identifier against all session file stems.
/// Returns the best match (highest score) along with the matched stem.
fn resolve_session_fuzzy(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Option<(PathBuf, String)> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry.get(name)?,
        None => registry.get("claude")?,
    };

    let sessions = format.list_sessions(project);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(session_id, CaseMatching::Ignore, Normalization::Smart);

    let mut best: Option<(PathBuf, String, u32)> = None;

    for s in &sessions {
        if let Some(stem) = s.path.file_stem().and_then(|os| os.to_str()) {
            let mut buf = Vec::new();
            if let Some(score) =
                pattern.score(nucleo_matcher::Utf32Str::new(stem, &mut buf), &mut matcher)
                && best
                    .as_ref()
                    .is_none_or(|(_, _, best_score)| score > *best_score)
            {
                best = Some((s.path.clone(), stem.to_string(), score));
            }
        }
    }

    best.map(|(path, stem, _)| (path, stem))
}

/// Check if a session file matches a grep pattern.
/// Searches through the raw JSONL content for any match.
pub(crate) fn session_matches_grep(path: &Path, pattern: &regex::Regex) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if pattern.is_match(&line) {
            return true;
        }
    }
    false
}

/// Get the Claude Code sessions directory for a project.
#[cfg(feature = "sessions-web")]
pub(crate) fn get_sessions_dir(project: Option<&Path>) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let claude_dir = PathBuf::from(home).join(".claude/projects");

    // Helper to convert a path to Claude's format: /home/user/foo -> -home-user-foo
    let path_to_claude_dir = |path: &Path| -> Option<PathBuf> {
        let path_str = path.to_string_lossy().replace('/', "-");
        // Try with leading dash first (Claude's format)
        let proj_dir = claude_dir.join(format!("-{}", path_str.trim_start_matches('-')));
        if proj_dir.exists() {
            return Some(proj_dir);
        }
        // Try without leading dash
        let proj_dir = claude_dir.join(&path_str);
        if proj_dir.exists() {
            return Some(proj_dir);
        }
        None
    };

    // 1. Explicit project path
    if let Some(proj) = project
        && let Some(dir) = path_to_claude_dir(proj)
    {
        return Some(dir);
    }

    // 2. Git root of current directory
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        && output.status.success()
        && let Some(dir) =
            path_to_claude_dir(Path::new(String::from_utf8_lossy(&output.stdout).trim()))
    {
        return Some(dir);
    }

    // 3. Current directory
    if let Ok(cwd) = std::env::current_dir()
        && let Some(dir) = path_to_claude_dir(&cwd)
    {
        return Some(dir);
    }

    None
}
