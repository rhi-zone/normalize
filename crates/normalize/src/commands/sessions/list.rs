//! List sessions command.

use super::format_age;
use crate::output::OutputFormatter;
use crate::sessions::{ContentBlock, FormatRegistry, LogFormat, Role, SessionFile};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A session in the list
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct SessionListItem {
    id: String,
    path: PathBuf,
    age_seconds: u64,
    /// Decoded project name (last path component) from the session's parent directory.
    project: Option<String>,
    first_message: Option<String>,
    user_messages: usize,
    tool_calls: usize,
    duration_seconds: Option<u64>,
    /// Parent session ID (set when this is a subagent session).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_id: Option<String>,
    /// Agent ID (set for subagent sessions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent_id: Option<String>,
    /// Subagent type (e.g. "general-purpose", "Explore").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subagent_type: Option<String>,
}

/// Session list report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SessionListReport {
    sessions: Vec<SessionListItem>,
    /// Present when `--limit` truncated the results.
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<super::TruncationInfo>,
    /// Whether any subagent sessions are included (controls column display).
    #[serde(skip)]
    #[schemars(skip)]
    has_subagents: bool,
}

impl SessionListReport {
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Returns true if sessions span more than one project.
    fn is_multi_project(&self) -> bool {
        let projects: HashSet<Option<&str>> =
            self.sessions.iter().map(|s| s.project.as_deref()).collect();
        projects.len() > 1
    }
}

impl OutputFormatter for SessionListReport {
    fn format_text(&self) -> String {
        let multi = self.is_multi_project();
        let mut lines = Vec::new();

        // Legend header
        let legend = if self.has_subagents && multi {
            "# id  age  duration  user_msgs  tools  project  parent  type  first_message"
                .to_string()
        } else if self.has_subagents {
            "# id  age  duration  user_msgs  tools  parent  type  first_message".to_string()
        } else if multi {
            "# id  age  duration  user_msgs  tools  project  first_message".to_string()
        } else {
            "# id  age  duration  user_msgs  tools  first_message".to_string()
        };
        lines.push(legend);

        for item in &self.sessions {
            let age = format_age(item.age_seconds);
            let duration = item
                .duration_seconds
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string());
            let user_msgs = item.user_messages.to_string();
            let tool_calls = item.tool_calls.to_string();
            let first_message = item
                .first_message
                .as_deref()
                .map(truncate_message)
                .unwrap_or_default();

            let mut parts = vec![item.id.clone(), age, duration, user_msgs, tool_calls];
            if multi {
                parts.push(item.project.as_deref().unwrap_or("?").to_string());
            }
            if self.has_subagents {
                parts.push(item.parent_id.as_deref().unwrap_or("-").to_string());
                parts.push(item.subagent_type.as_deref().unwrap_or("-").to_string());
            }
            parts.push(first_message);
            lines.push(parts.join("  "));
        }
        if let Some(ref t) = self.truncated {
            lines.push(t.notice());
        }
        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color::{Cyan, DarkGray, Green, Magenta, Yellow};
        let multi = self.is_multi_project();
        let mut lines = Vec::new();

        for item in &self.sessions {
            let age = format_age(item.age_seconds);
            let duration = item
                .duration_seconds
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string());
            let title = item
                .first_message
                .as_deref()
                .map(truncate_message)
                .unwrap_or_default();

            let mut parts = vec![
                Green.paint(item.id.as_str()).to_string(),
                DarkGray.paint(age).to_string(),
                Cyan.paint(duration).to_string(),
                format!(
                    "{}  {}",
                    Yellow.paint(format!("{} user", item.user_messages)),
                    Yellow.paint(format!("{} tools", item.tool_calls))
                ),
            ];
            if multi {
                let proj = item.project.as_deref().unwrap_or("?");
                parts.push(Cyan.bold().paint(proj).to_string());
            }
            if self.has_subagents {
                let parent = item.parent_id.as_deref().unwrap_or("-");
                parts.push(Magenta.paint(parent).to_string());
                let agent_type = item.subagent_type.as_deref().unwrap_or("-");
                parts.push(DarkGray.paint(agent_type).to_string());
            }
            parts.push(title);

            lines.push(parts.join("  "));
        }
        if let Some(ref t) = self.truncated {
            use nu_ansi_term::Color::DarkGray;
            lines.push(DarkGray.paint(t.notice()).to_string());
        }
        lines.join("\n")
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

fn truncate_message(s: &str) -> String {
    let s = s.trim().replace('\n', " ");
    if s.len() > 72 {
        format!("{}…", &s[..72])
    } else {
        s
    }
}

/// Decode the project name from a session's parent directory name.
/// Claude encodes project paths by replacing '/' with '-' and prepending '-'.
/// Returns the last path component as a short project name.
pub(crate) fn project_from_path(path: &Path) -> Option<String> {
    let dir_name = path.parent()?.file_name()?.to_str()?;
    // Strip leading dash, split on dash, take the last non-empty segment
    let stripped = dir_name.trim_start_matches('-');
    // Use the last segment as the project name (best-effort, since '-' is ambiguous)
    stripped
        .split('-')
        .filter(|s| !s.is_empty())
        .next_back()
        .map(String::from)
}

struct SessionStats {
    name: Option<String>,
    message_count: usize,
    tool_use_count: usize,
    first_timestamp: Option<u64>,
}

/// Extract rich stats from a session file.
fn extract_session_stats(format: &dyn LogFormat, path: &Path) -> SessionStats {
    let Ok(session) = format.parse(path) else {
        return SessionStats {
            name: None,
            message_count: 0,
            tool_use_count: 0,
            first_timestamp: None,
        };
    };

    let first_message = session
        .turns
        .iter()
        .flat_map(|t| &t.messages)
        .find(|m| m.role == Role::User)
        .and_then(|m| {
            m.content.iter().find_map(|c| match c {
                ContentBlock::Text { text } => Some(text.clone()),
                _ => None,
            })
        });

    let user_messages = session.messages_by_role(Role::User);
    let tool_calls = session.tool_uses().count();

    // Duration from first to last message timestamp
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

    SessionStats {
        name: first_message,
        message_count: user_messages,
        tool_use_count: tool_calls,
        first_timestamp: duration_seconds,
    }
}

/// Parse two RFC 3339 timestamps and return the difference in seconds.
fn parse_rfc3339_diff(first: &str, last: &str) -> Option<u64> {
    use chrono::DateTime;
    let a = DateTime::parse_from_rfc3339(first).ok()?;
    let b = DateTime::parse_from_rfc3339(last).ok()?;
    let diff = (b - a).num_seconds();
    if diff > 0 { Some(diff as u64) } else { None }
}

/// Build a session list report (data only, no printing).
#[allow(clippy::too_many_arguments)]
pub fn build_session_list(
    project: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    mode: &super::SessionMode,
    agent_type: Option<&str>,
) -> Result<SessionListReport, String> {
    use super::stats::{list_all_project_sessions_by_mode, parse_date};
    use std::time::{Duration, SystemTime};

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
        let proj = project_filter.or(project);
        super::list_sessions_by_mode(format, proj, mode)
    };

    // Date filtering
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

    if let Some(ref re) = grep_re {
        sessions.retain(|s| super::session_matches_grep(&s.path, re));
    }

    // Agent type filtering (case-insensitive match on subagent_type)
    if let Some(at) = agent_type {
        let at_lower = at.to_lowercase();
        sessions.retain(|s| {
            s.subagent_type
                .as_deref()
                .is_some_and(|t| t.to_lowercase() == at_lower)
        });
    }

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    let total_before_limit = sessions.len();
    if limit > 0 {
        sessions.truncate(limit);
    }
    let truncated = super::TruncationInfo::if_truncated(total_before_limit, limit);

    let has_subagents = sessions.iter().any(|s| s.parent_id.is_some());

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
            let project = project_from_path(&s.path);
            let stats = extract_session_stats(format, &s.path);
            SessionListItem {
                id,
                path: s.path.clone(),
                age_seconds,
                project,
                first_message: stats.name,
                user_messages: stats.message_count,
                tool_calls: stats.tool_use_count,
                duration_seconds: stats.first_timestamp,
                parent_id: s.parent_id.clone(),
                agent_id: s.agent_id.clone(),
                subagent_type: s.subagent_type.clone(),
            }
        })
        .collect();

    Ok(SessionListReport {
        sessions: items,
        truncated,
        has_subagents,
    })
}
