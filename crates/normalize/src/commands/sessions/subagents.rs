//! Subagent listing and summary for a parent session.

use crate::output::OutputFormatter;
use crate::sessions::{FormatRegistry, LogFormat, Session, list_subagent_sessions};
use serde::Serialize;
use std::fmt::Write as _;
use std::path::Path;

/// Summary of a single subagent within a parent session.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SubagentSummaryItem {
    pub agent_id: String,
    pub subagent_type: Option<String>,
    pub turns: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_seconds: Option<u64>,
    pub tool_calls: usize,
}

/// Report listing all subagents for a parent session.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SubagentsReport {
    pub parent_id: String,
    pub subagents: Vec<SubagentSummaryItem>,
}

impl OutputFormatter for SubagentsReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# Subagents for session {}", self.parent_id);
        let _ = writeln!(
            out,
            "# agent_id  type  turns  tokens_in  tokens_out  duration  tools"
        );
        for item in &self.subagents {
            let agent_type = item.subagent_type.as_deref().unwrap_or("-");
            let duration = item
                .duration_seconds
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string());
            let _ = writeln!(
                out,
                "{}  {}  {}t  {}in  {}out  {}  {}tc",
                item.agent_id,
                agent_type,
                item.turns,
                format_tokens(item.input_tokens),
                format_tokens(item.output_tokens),
                duration,
                item.tool_calls,
            );
        }
        out
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color::{Cyan, Green, Yellow};
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{}",
            Green
                .bold()
                .paint(format!("# Subagents for session {}", self.parent_id))
        );
        for item in &self.subagents {
            let agent_type = item.subagent_type.as_deref().unwrap_or("-");
            let duration = item
                .duration_seconds
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string());
            let _ = writeln!(
                out,
                "{}  {}  {}  {}  {}",
                Green.paint(&item.agent_id),
                Yellow.paint(agent_type),
                Cyan.paint(format!(
                    "{}t  {}in {}out",
                    item.turns,
                    format_tokens(item.input_tokens),
                    format_tokens(item.output_tokens),
                )),
                Cyan.paint(duration),
                Yellow.paint(format!("{} tools", item.tool_calls)),
            );
        }
        out
    }
}

impl std::fmt::Display for SubagentsReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

/// Build a subagents report for a given parent session ID.
pub fn build_subagents_report(
    session_id: &str,
    project: Option<&Path>,
    format_name: Option<&str>,
) -> Result<SubagentsReport, String> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").ok_or_else(|| {
            "Claude format not available (compile with feature = format-claude)".to_string()
        })?,
    };

    // Find the sessions dir and look for subagents under this session
    let sessions_dir = format.sessions_dir(project);

    // Try to find the session directory (exact or prefix match)
    let session_dir = find_session_dir(&sessions_dir, session_id)
        .ok_or_else(|| format!("No session directory found for: {}", session_id))?;

    let parent_id = session_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(session_id)
        .to_string();

    let subagent_files = list_subagent_sessions(&sessions_dir);
    let subagent_files: Vec<_> = subagent_files
        .into_iter()
        .filter(|s| s.parent_id.as_deref() == Some(&parent_id))
        .collect();

    let mut items = Vec::new();
    for sf in &subagent_files {
        let session = format
            .parse(&sf.path)
            .unwrap_or_else(|_| Session::new(sf.path.clone(), format.name()));
        let tokens = session.total_tokens();
        let tool_calls = session.tool_uses().count();

        // Duration from timestamps
        let timestamps: Vec<&str> = session
            .turns
            .iter()
            .flat_map(|t| &t.messages)
            .filter_map(|m| m.timestamp.as_deref())
            .collect();
        let duration_seconds = if timestamps.len() >= 2 {
            let first = timestamps.iter().copied().min();
            let last = timestamps.iter().copied().max();
            if let (Some(first), Some(last)) = (first, last) {
                parse_rfc3339_diff(first, last)
            } else {
                None
            }
        } else {
            None
        };

        items.push(SubagentSummaryItem {
            agent_id: sf.agent_id.clone().unwrap_or_else(|| "unknown".to_string()),
            subagent_type: sf.subagent_type.clone(),
            turns: session.turns.len(),
            input_tokens: tokens.input,
            output_tokens: tokens.output,
            duration_seconds,
            tool_calls,
        });
    }

    Ok(SubagentsReport {
        parent_id,
        subagents: items,
    })
}

/// Find a session directory by exact match or prefix.
fn find_session_dir(sessions_dir: &Path, session_id: &str) -> Option<std::path::PathBuf> {
    // Exact match
    let exact = sessions_dir.join(session_id);
    if exact.is_dir() {
        return Some(exact);
    }
    // Prefix match
    if let Ok(entries) = std::fs::read_dir(sessions_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with(session_id)
            {
                return Some(path);
            }
        }
    }
    None
}

/// Parse two RFC 3339 timestamps and return the difference in seconds.
fn parse_rfc3339_diff(first: &str, last: &str) -> Option<u64> {
    use chrono::DateTime;
    let a = DateTime::parse_from_rfc3339(first).ok()?;
    let b = DateTime::parse_from_rfc3339(last).ok()?;
    let diff = (b - a).num_seconds();
    if diff > 0 { Some(diff as u64) } else { None }
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{}", tokens)
    }
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{}h", h)
        } else {
            format!("{}h{}m", h, m)
        }
    }
}
