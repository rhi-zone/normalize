//! List sessions command.

use super::{format_age, session_matches_grep};
use crate::output::OutputFormatter;
use crate::sessions::{FormatRegistry, LogFormat, SessionFile};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// A session in the list
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct SessionListItem {
    id: String,
    path: PathBuf,
    age_seconds: u64,
}

/// Session list report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SessionListReport {
    sessions: Vec<SessionListItem>,
}

impl OutputFormatter for SessionListReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        for item in &self.sessions {
            let age = format_age(item.age_seconds);
            lines.push(format!("{} ({})", item.id, age));
        }
        lines.join("\n")
    }
}

/// List available sessions for a format.
pub fn cmd_sessions_list(
    project: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    output_format: &crate::output::OutputFormat,
) -> i32 {
    let json = output_format.is_json();
    let registry = FormatRegistry::new();

    // Get format (default to claude for backwards compatibility)
    let format: &dyn LogFormat = match format_name {
        Some(name) => match registry.get(name) {
            Some(f) => f,
            None => {
                eprintln!("Unknown format: {}", name);
                return 1;
            }
        },
        None => registry.get("claude").unwrap(),
    };

    // Compile grep pattern if provided
    let grep_re = grep.and_then(|p| regex::Regex::new(p).ok());
    if grep.is_some() && grep_re.is_none() {
        eprintln!("Invalid grep pattern: {}", grep.unwrap());
        return 1;
    }

    // Get sessions from format (handles directory structure differences)
    let mut sessions: Vec<SessionFile> = format.list_sessions(project);

    // Apply grep filter if provided
    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    sessions.truncate(limit);

    if sessions.is_empty() {
        eprintln!("No {} sessions found", format_name.unwrap_or("Claude Code"));
        return 0;
    }

    let items: Vec<SessionListItem> = sessions
        .iter()
        .map(|s| {
            let id = s
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let age_seconds = s.mtime.elapsed().map(|d| d.as_secs()).unwrap_or(0);
            SessionListItem {
                id,
                path: s.path.clone(),
                age_seconds,
            }
        })
        .collect();

    let report = SessionListReport { sessions: items };
    let config = crate::config::NormalizeConfig::default();
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);

    0
}
