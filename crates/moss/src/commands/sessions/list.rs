//! List sessions command.

use super::{format_age, session_matches_grep};
use crate::sessions::{FormatRegistry, LogFormat, SessionFile};
use std::path::Path;

/// List available sessions for a format.
pub fn cmd_sessions_list(
    project: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    json: bool,
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

    // Get sessions from format (handles directory structure differences)
    let mut sessions: Vec<SessionFile> = format.list_sessions(project);

    // Apply grep filter if provided
    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    sessions.truncate(limit);

    if sessions.is_empty() {
        if json {
            println!("[]");
        } else {
            eprintln!("No {} sessions found", format_name.unwrap_or("Claude Code"));
        }
        return 0;
    }

    if json {
        let output: Vec<_> = sessions
            .iter()
            .map(|s| {
                let id = s.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let age = s.mtime.elapsed().map(|d| d.as_secs()).unwrap_or(0);
                serde_json::json!({
                    "id": id,
                    "path": s.path,
                    "age_seconds": age
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&output).unwrap());
    } else {
        for s in &sessions {
            let id = s
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("");
            let age = format_age(s.mtime.elapsed().map(|d| d.as_secs()).unwrap_or(0));
            println!("{} ({})", id, age);
        }
    }

    0
}
