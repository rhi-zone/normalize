use super::list::project_from_path;
use super::stats::{list_all_project_sessions, parse_date};
use crate::output::OutputFormatter;
use crate::sessions::{ContentBlock, FormatRegistry, LogFormat, SessionFile};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// A single matching line from a session message.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GrepMatch {
    pub session_id: String,
    pub project: Option<String>,
    pub turn: usize,
    pub role: String,
    pub timestamp: Option<String>,
    pub line_num: usize,
    pub line: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// Report for `sessions grep`.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SessionGrepReport {
    pub matches: Vec<GrepMatch>,
    pub total_sessions_searched: usize,
    pub pretty: bool,
    pub context_lines: usize,
}

impl OutputFormatter for SessionGrepReport {
    fn format_text(&self) -> String {
        if self.matches.is_empty() {
            return "No matches found.".to_string();
        }

        let mut lines = Vec::new();
        let mut last_key: Option<(String, usize)> = None;

        for m in &self.matches {
            let id_short = if m.session_id.len() > 8 {
                &m.session_id[..8]
            } else {
                &m.session_id
            };
            let key = (m.session_id.clone(), m.turn);
            if last_key.as_ref() != Some(&key) {
                if last_key.is_some() {
                    lines.push(String::new());
                }
                let ts = m.timestamp.as_deref().unwrap_or("?");
                let ts_short = if ts.len() > 19 { &ts[..19] } else { ts };
                let ts_display = ts_short.replace('T', " ");
                lines.push(format!(
                    "[{}] turn {} ({}, {})",
                    id_short, m.turn, m.role, ts_display
                ));
                last_key = Some(key);
            }

            for (i, ctx) in m.context_before.iter().enumerate() {
                let ctx_line_num = m.line_num - m.context_before.len() + i;
                lines.push(format!("  {:>4}-  {}", ctx_line_num + 1, ctx));
            }
            lines.push(format!("  {:>4}:  {}", m.line_num + 1, m.line));
            for (i, ctx) in m.context_after.iter().enumerate() {
                lines.push(format!("  {:>4}-  {}", m.line_num + 1 + i + 1, ctx));
            }
        }

        lines.push(format!(
            "--- {} match(es) in {} session(s) searched ---",
            self.matches.len(),
            self.total_sessions_searched
        ));

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        if self.matches.is_empty() {
            return "\x1b[33mNo matches found.\x1b[0m".to_string();
        }

        let mut lines = Vec::new();
        let mut last_key: Option<(String, usize)> = None;

        for m in &self.matches {
            let id_short = if m.session_id.len() > 8 {
                &m.session_id[..8]
            } else {
                &m.session_id
            };
            let key = (m.session_id.clone(), m.turn);
            if last_key.as_ref() != Some(&key) {
                if last_key.is_some() {
                    lines.push(String::new());
                }
                let ts = m.timestamp.as_deref().unwrap_or("?");
                let ts_short = if ts.len() > 19 { &ts[..19] } else { ts };
                let ts_display = ts_short.replace('T', " ");
                let role_badge = match m.role.as_str() {
                    "user" => "\x1b[34m[user]\x1b[0m",
                    "assistant" => "\x1b[32m[asst]\x1b[0m",
                    "system" => "\x1b[33m[sys]\x1b[0m",
                    "tool" => "\x1b[35m[tool]\x1b[0m",
                    _ => &m.role,
                };
                let project_tag = m
                    .project
                    .as_deref()
                    .map(|p| format!(" \x1b[36m{}\x1b[0m", p))
                    .unwrap_or_default();
                lines.push(format!(
                    "\x1b[33m{}\x1b[0m {} \x1b[90m{}\x1b[0m{} turn {}",
                    id_short, role_badge, ts_display, project_tag, m.turn
                ));
                last_key = Some(key);
            }

            for (i, ctx) in m.context_before.iter().enumerate() {
                let ctx_line_num = m.line_num - m.context_before.len() + i;
                lines.push(format!(
                    "  \x1b[90m{:>4}-  {}\x1b[0m",
                    ctx_line_num + 1,
                    ctx
                ));
            }
            lines.push(format!(
                "  \x1b[32m{:>4}\x1b[0m:  {}",
                m.line_num + 1,
                m.line
            ));
            for (i, ctx) in m.context_after.iter().enumerate() {
                lines.push(format!(
                    "  \x1b[90m{:>4}-  {}\x1b[0m",
                    m.line_num + 1 + i + 1,
                    ctx
                ));
            }
        }

        lines.push(format!(
            "\x1b[1m--- {} match(es) in {} session(s) searched ---\x1b[0m",
            self.matches.len(),
            self.total_sessions_searched
        ));

        lines.join("\n")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_grep_report(
    root: Option<&Path>,
    pattern: &str,
    context_lines: usize,
    role: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    format_name: Option<&str>,
    limit: usize,
    ignore_case: bool,
    pretty: bool,
) -> Result<SessionGrepReport, String> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").ok_or_else(|| {
            "Claude format not available (compile with feature = format-claude)".to_string()
        })?,
    };

    let re_pattern = if ignore_case {
        format!("(?i){}", pattern)
    } else {
        pattern.to_string()
    };
    let re = regex::Regex::new(&re_pattern).map_err(|e| format!("Invalid pattern: {}", e))?;

    let mut sessions: Vec<SessionFile> = if all_projects {
        list_all_project_sessions(format)
    } else {
        let proj = project_filter.or(root);
        format.list_sessions(proj)
    };

    let now = SystemTime::now();
    if let Some(d) = days {
        let since_time = now - Duration::from_secs(d as u64 * 86400);
        sessions.retain(|s| s.mtime >= since_time);
    }
    if let Some(s) = since {
        if let Some(since_time) = parse_date(s) {
            sessions.retain(|s| s.mtime >= since_time);
        } else {
            return Err(format!("Invalid date format: {} (use YYYY-MM-DD)", s));
        }
    }
    if let Some(u) = until {
        if let Some(until_time) = parse_date(u) {
            let until_time = until_time + Duration::from_secs(86400);
            sessions.retain(|s| s.mtime <= until_time);
        } else {
            return Err(format!("Invalid date format: {} (use YYYY-MM-DD)", u));
        }
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    let total_sessions_searched = sessions.len();
    let mut matches = Vec::new();

    'session: for sf in &sessions {
        let Ok(session) = format.parse(&sf.path) else {
            continue;
        };

        let session_id = session
            .metadata
            .session_id
            .clone()
            .or_else(|| {
                sf.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            })
            .unwrap_or_default();
        let project = project_from_path(&sf.path);

        for (turn_idx, turn) in session.turns.iter().enumerate() {
            for msg in &turn.messages {
                let role_str = msg.role.to_string();

                if let Some(r) = role {
                    match r {
                        "user" if role_str != "user" => continue,
                        "assistant" if role_str != "assistant" => continue,
                        _ => {}
                    }
                }

                let text: String = msg
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if text.is_empty() {
                    continue;
                }

                let msg_lines: Vec<&str> = text.lines().collect();

                for (line_idx, line) in msg_lines.iter().enumerate() {
                    if !re.is_match(line) {
                        continue;
                    }

                    let before_start = line_idx.saturating_sub(context_lines);
                    let context_before: Vec<String> = msg_lines[before_start..line_idx]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();

                    let after_end = (line_idx + 1 + context_lines).min(msg_lines.len());
                    let context_after: Vec<String> = msg_lines[line_idx + 1..after_end]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();

                    matches.push(GrepMatch {
                        session_id: session_id.clone(),
                        project: project.clone(),
                        turn: turn_idx,
                        role: role_str.clone(),
                        timestamp: msg.timestamp.clone(),
                        line_num: line_idx,
                        line: line.to_string(),
                        context_before,
                        context_after,
                    });

                    // Limit total matches to avoid flooding output
                    if matches.len() >= 1000 {
                        break 'session;
                    }
                }
            }
        }
    }

    Ok(SessionGrepReport {
        matches,
        total_sessions_searched,
        pretty,
        context_lines,
    })
}
