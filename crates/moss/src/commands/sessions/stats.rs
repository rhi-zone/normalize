//! Aggregate statistics across sessions.

use super::{analyze::cmd_sessions_analyze_multi, session_matches_grep};
use crate::sessions::{FormatRegistry, LogFormat, SessionFile};
use std::path::Path;

/// Show aggregate statistics across all sessions.
pub fn cmd_sessions_stats(
    project: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    json: bool,
    pretty: bool,
) -> i32 {
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
    let grep_re = grep.map(|p| regex::Regex::new(p).ok()).flatten();
    if grep.is_some() && grep_re.is_none() {
        eprintln!("Invalid grep pattern: {}", grep.unwrap());
        return 1;
    }

    // Get sessions from format
    let mut sessions: Vec<SessionFile> = format.list_sessions(project);

    // Apply grep filter if provided
    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }

    // Sort by time (newest first) and limit
    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    sessions.truncate(limit);

    if sessions.is_empty() {
        if json {
            println!("{{}}");
        } else {
            eprintln!("No {} sessions found", format_name.unwrap_or("Claude Code"));
        }
        return 0;
    }

    // Collect paths and analyze
    let paths: Vec<_> = sessions.iter().map(|s| s.path.clone()).collect();
    cmd_sessions_analyze_multi(&paths, format_name, json, pretty)
}
