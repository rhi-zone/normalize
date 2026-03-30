//! Sessions command - analyze Claude Code and other agent session logs.

pub mod analyze;
pub mod list;
pub mod messages;
pub mod patterns;
pub mod plans;
#[cfg(feature = "sessions-web")]
mod serve;
pub mod show;
pub mod sort;
pub mod stats;

pub use list::{SessionListReport, build_session_list};
pub use messages::{MessagesReport, build_messages_report};
pub use patterns::{PatternsReport, build_patterns_report};
pub use plans::{PlanContent, PlansListReport, build_plan_content, build_plans_list};
#[cfg(feature = "sessions-web")]
pub use serve::serve_sessions;
pub use show::{SessionShowReport, build_analyze_report, build_show_report};
pub use stats::{build_stats_data, show_stats_grouped};

pub mod subagents;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::sessions::{FormatRegistry, LogFormat, SessionFile};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

pub use subagents::{SubagentSummaryItem, SubagentsReport};

/// Metadata about truncation applied by `--limit`.
/// Included in reports so text/pretty output can show a notice and JSON output
/// can carry structured truncation info.
#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct TruncationInfo {
    /// Number of items shown after truncation.
    pub showing: usize,
    /// Total number of items before truncation.
    pub total: usize,
}

impl TruncationInfo {
    /// Create a `TruncationInfo` only when truncation actually occurred.
    pub fn if_truncated(total: usize, limit: usize) -> Option<Self> {
        if limit > 0 && total > limit {
            Some(Self {
                showing: limit,
                total,
            })
        } else {
            None
        }
    }

    /// Format a human-readable notice for text/pretty output.
    pub fn notice(&self) -> String {
        format!(
            "Showing {} of {} sessions (use --limit 0 for all)",
            self.showing, self.total
        )
    }
}

/// Session filter mode: which kinds of sessions to include.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    /// Only top-level interactive sessions (default).
    #[default]
    Interactive,
    /// Only subagent sessions.
    Subagent,
    /// Both interactive and subagent sessions.
    All,
}

impl std::str::FromStr for SessionMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Support comma-delimited: "interactive,subagent" => All
        let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
        let mut interactive = false;
        let mut subagent = false;
        for part in &parts {
            match *part {
                "interactive" => interactive = true,
                "subagent" | "subagents" => subagent = true,
                "all" => {
                    interactive = true;
                    subagent = true;
                }
                other => {
                    return Err(format!(
                        "Unknown mode: {} (expected interactive, subagent, or all)",
                        other
                    ));
                }
            }
        }
        if interactive && subagent {
            Ok(Self::All)
        } else if subagent {
            Ok(Self::Subagent)
        } else {
            Ok(Self::Interactive)
        }
    }
}

impl std::fmt::Display for SessionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interactive => write!(f, "interactive"),
            Self::Subagent => write!(f, "subagent"),
            Self::All => write!(f, "all"),
        }
    }
}

/// List sessions filtered by mode.
pub(crate) fn list_sessions_by_mode(
    format: &dyn LogFormat,
    project: Option<&Path>,
    mode: &SessionMode,
) -> Vec<SessionFile> {
    match mode {
        SessionMode::Interactive => format.list_sessions(project),
        SessionMode::Subagent => format.list_subagent_sessions(project),
        SessionMode::All => {
            let mut all = format.list_sessions(project);
            all.extend(format.list_subagent_sessions(project));
            all
        }
    }
}

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
/// Also searches subagent directories for agent IDs (e.g. "agent-a5c5ccc9c2b61e757").
pub(crate) fn resolve_session_paths(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Vec<PathBuf> {
    let literal = resolve_session_paths_literal(session_id, project, format_name);
    if !literal.is_empty() {
        return literal;
    }

    // Try resolving as a subagent ID
    if let Some(path) = resolve_subagent_path(session_id, project, format_name) {
        return vec![path];
    }

    // Fuzzy fallback
    if let Some((path, matched_stem)) = resolve_session_fuzzy(session_id, project, format_name) {
        eprintln!("fuzzy match: {}", matched_stem);
        return vec![path];
    }

    Vec::new()
}

/// Try to resolve a subagent ID to its JSONL path.
/// Searches through all session subdirectories for matching agent files.
fn resolve_subagent_path(
    agent_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Option<PathBuf> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry.get(name)?,
        None => registry.get("claude")?,
    };

    let subagent_sessions = format.list_subagent_sessions(project);
    for s in &subagent_sessions {
        if let Some(stem) = s.path.file_stem().and_then(|os| os.to_str())
            && (stem == agent_id || stem.starts_with(agent_id))
        {
            return Some(s.path.clone());
        }
        // Also match by agent_id field
        if let Some(ref aid) = s.agent_id
            && (aid == agent_id || aid.starts_with(agent_id))
        {
            return Some(s.path.clone());
        }
    }
    None
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

    // 2. Git root of current directory (via gix — no PATH dependency)
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(repo) = gix::discover(&cwd)
        && let Some(worktree) = repo.workdir()
        && let Some(dir) = path_to_claude_dir(worktree)
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
