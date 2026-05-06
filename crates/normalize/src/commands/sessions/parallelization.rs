//! Parallelization hints: find sequential independent tool calls that could run in parallel.

use crate::output::OutputFormatter;
use crate::sessions::{
    ContentBlock, FormatRegistry, LogFormat, Role, SessionFile, parse_session,
    parse_session_with_format,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::stats::{list_all_project_sessions_by_mode, parse_date};
use super::{SessionMode, list_sessions_by_mode, session_matches_grep};

/// A group of sequential same-type tool calls within a turn that could be parallelized.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ParallelGroup {
    /// Turn index (1-based for display).
    pub turn: usize,
    /// Tool name (e.g. "Read", "Bash", "Grep").
    pub tool: String,
    /// The targets (file paths, commands, patterns, etc.) for each call.
    pub targets: Vec<String>,
    /// Number of API round-trips that could be saved.
    pub savings: usize,
}

/// Report for `normalize sessions parallelization`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ParallelizationReport {
    pub session_path: PathBuf,
    pub groups: Vec<ParallelGroup>,
    pub total_turns: usize,
    pub total_savings: usize,
}

impl OutputFormatter for ParallelizationReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if self.groups.is_empty() {
            writeln!(out, "No parallelization opportunities found.").unwrap();
            return out;
        }
        writeln!(
            out,
            "Parallelization opportunities: {} groups, {} potential round-trips saved",
            self.groups.len(),
            self.total_savings
        )
        .unwrap();
        writeln!(out).unwrap();
        for g in &self.groups {
            let calls: Vec<String> = g
                .targets
                .iter()
                .map(|t| format!("{}({})", g.tool, t))
                .collect();
            writeln!(
                out,
                "  Turn {:>3}: Could parallelize: {}",
                g.turn,
                calls.join(" → ")
            )
            .unwrap();
        }
        out
    }

    fn format_pretty(&self) -> String {
        let mut out = String::new();
        if self.groups.is_empty() {
            writeln!(out, "\x1b[2mNo parallelization opportunities found.\x1b[0m").unwrap();
            return out;
        }
        writeln!(
            out,
            "\x1b[1mParallelization Opportunities\x1b[0m — {} groups, \x1b[33m{} round-trips saved\x1b[0m",
            self.groups.len(),
            self.total_savings
        )
        .unwrap();
        writeln!(out).unwrap();
        for g in &self.groups {
            let calls: Vec<String> = g
                .targets
                .iter()
                .map(|t| format!("\x1b[36m{}({})\x1b[0m", g.tool, t))
                .collect();
            writeln!(
                out,
                "  Turn \x1b[2m{:>3}\x1b[0m: {}",
                g.turn,
                calls.join(" \x1b[2m→\x1b[0m ")
            )
            .unwrap();
        }
        out
    }
}

/// Extract a short target description from a tool call's input.
fn extract_target(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(short_path)
            .unwrap_or_else(|| "?".into()),
        "Edit" | "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(short_path)
            .unwrap_or_else(|| "?".into()),
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|c| {
                let s = c.trim();
                if s.len() > 60 {
                    format!("{}…", &s[..60])
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "?".into()),
        "Grep" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| p.chars().take(40).collect())
            .unwrap_or_else(|| "?".into()),
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| p.chars().take(40).collect())
            .unwrap_or_else(|| "?".into()),
        _ => input
            .as_object()
            .and_then(|o| o.values().next())
            .and_then(|v| v.as_str())
            .map(|s| s.chars().take(40).collect())
            .unwrap_or_else(|| "?".into()),
    }
}

fn short_path(p: &str) -> String {
    // Show last two path components
    let parts: Vec<&str> = p.trim_end_matches('/').split('/').collect();
    if parts.len() <= 2 {
        p.to_string()
    } else {
        parts[parts.len() - 2..].join("/")
    }
}

/// Analyze a single session file for parallelization opportunities.
pub fn analyze_parallelization(
    path: &Path,
    format_name: Option<&str>,
    threshold: usize,
) -> Option<ParallelizationReport> {
    let session = if let Some(fmt) = format_name {
        parse_session_with_format(path, fmt).ok()?
    } else {
        parse_session(path).ok()?
    };

    let mut groups: Vec<ParallelGroup> = Vec::new();
    let total_turns = session.turns.len();

    for (turn_idx, turn) in session.turns.iter().enumerate() {
        // Collect all tool calls in this turn from assistant messages.
        let mut tool_calls: Vec<(String, String)> = Vec::new(); // (tool_name, target)

        for msg in &turn.messages {
            if msg.role == Role::Assistant {
                for block in &msg.content {
                    if let ContentBlock::ToolUse { name, input, .. } = block {
                        let target = extract_target(name, input);
                        tool_calls.push((name.clone(), target));
                    }
                }
            }
        }

        if tool_calls.len() < threshold {
            continue;
        }

        // Find runs of same tool name with different targets.
        let mut i = 0;
        while i < tool_calls.len() {
            let tool = &tool_calls[i].0;
            let mut j = i + 1;
            while j < tool_calls.len() && &tool_calls[j].0 == tool {
                j += 1;
            }
            let run_len = j - i;
            if run_len >= threshold {
                // Check targets are all distinct (a simple proxy for independence).
                let targets: Vec<String> =
                    tool_calls[i..j].iter().map(|(_, t)| t.clone()).collect();
                let unique: std::collections::HashSet<_> = targets.iter().collect();
                if unique.len() >= threshold {
                    groups.push(ParallelGroup {
                        turn: turn_idx + 1,
                        tool: tool.clone(),
                        targets,
                        savings: run_len - 1,
                    });
                }
            }
            i = j;
        }
    }

    let total_savings = groups.iter().map(|g| g.savings).sum();
    Some(ParallelizationReport {
        session_path: path.to_path_buf(),
        groups,
        total_turns,
        total_savings,
    })
}

/// Build a parallelization report for a single session (by ID).
pub fn build_parallelization_report_for_session(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
    exact: bool,
    threshold: usize,
) -> Result<ParallelizationReport, String> {
    use super::{resolve_session_paths, resolve_session_paths_literal};

    let paths = if exact {
        resolve_session_paths_literal(session_id, project, format_name)
    } else {
        resolve_session_paths(session_id, project, format_name)
    };

    if paths.is_empty() {
        return Err(format!("No sessions found matching: {}", session_id));
    }

    let mut merged = ParallelizationReport {
        session_path: paths[0].clone(),
        groups: Vec::new(),
        total_turns: 0,
        total_savings: 0,
    };

    for path in &paths {
        if let Some(r) = analyze_parallelization(path, format_name, threshold) {
            merged.groups.extend(r.groups);
            merged.total_turns += r.total_turns;
        }
    }

    merged.total_savings = merged.groups.iter().map(|g| g.savings).sum();
    Ok(merged)
}

/// Build a parallelization report across multiple filtered sessions.
#[allow(clippy::too_many_arguments)]
pub fn build_parallelization_report(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    mode: &SessionMode,
    agent_type: Option<&str>,
    threshold: usize,
) -> Result<ParallelizationReport, String> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").ok_or_else(|| {
            "Claude format not available (compile with feature = format-claude)".to_string()
        })?,
    };

    let grep_re = grep
        .map(|p| regex::Regex::new(p).map_err(|_| format!("Invalid grep pattern: {}", p)))
        .transpose()?;

    let mut sessions: Vec<SessionFile> = if all_projects {
        list_all_project_sessions_by_mode(format, mode)
    } else {
        let project = project_filter.or(root);
        list_sessions_by_mode(format, project, mode)
    };

    let now = std::time::SystemTime::now();
    let since_time = if let Some(d) = days {
        Some(now - std::time::Duration::from_secs(d as u64 * 86400))
    } else if let Some(s) = since {
        Some(parse_date(s).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", s))?)
    } else {
        None
    };
    let until_time = if let Some(u) = until {
        Some(
            parse_date(u).ok_or_else(|| format!("Invalid date format: {} (use YYYY-MM-DD)", u))?
                + std::time::Duration::from_secs(86400),
        )
    } else {
        None
    };

    if let Some(since) = since_time {
        sessions.retain(|s| s.mtime >= since);
    }
    if let Some(until) = until_time {
        sessions.retain(|s| s.mtime <= until);
    }
    if let Some(ref re) = grep_re {
        sessions.retain(|s| session_matches_grep(&s.path, re));
    }
    if let Some(at) = agent_type {
        let at_lower = at.to_lowercase();
        sessions.retain(|s| {
            s.subagent_type
                .as_deref()
                .is_some_and(|t| t.to_lowercase() == at_lower)
        });
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    let mut merged = ParallelizationReport {
        session_path: PathBuf::from("."),
        groups: Vec::new(),
        total_turns: 0,
        total_savings: 0,
    };

    for sf in &sessions {
        if let Some(r) = analyze_parallelization(&sf.path, format_name, threshold) {
            merged.groups.extend(r.groups);
            merged.total_turns += r.total_turns;
        }
    }

    merged.total_savings = merged.groups.iter().map(|g| g.savings).sum();
    Ok(merged)
}
