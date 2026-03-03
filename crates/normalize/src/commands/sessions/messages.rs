//! Extract all messages across sessions into a flat, queryable form.

use super::list::project_from_path;
use super::stats::{list_all_project_sessions, parse_date};
use crate::output::OutputFormatter;
use crate::sessions::{ContentBlock, FormatRegistry, LogFormat, SessionFile, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, SystemTime};

/// Filter for message roles.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RoleFilter {
    /// Show only user messages (default)
    #[default]
    User,
    /// Show only assistant messages
    Assistant,
    /// Show all messages
    All,
}

impl FromStr for RoleFilter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(RoleFilter::User),
            "assistant" | "asst" => Ok(RoleFilter::Assistant),
            "all" => Ok(RoleFilter::All),
            _ => Err(format!(
                "invalid role '{}': expected 'user', 'assistant', or 'all'",
                s
            )),
        }
    }
}

impl fmt::Display for RoleFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoleFilter::User => write!(f, "user"),
            RoleFilter::Assistant => write!(f, "assistant"),
            RoleFilter::All => write!(f, "all"),
        }
    }
}

/// A single message extracted from a session.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MessageRecord {
    pub session_id: String,
    pub project: Option<String>,
    pub turn: usize,
    pub role: String,
    pub timestamp: Option<String>,
    pub text: String,
    pub char_count: usize,
    /// Token usage for this turn (present on assistant messages when available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,
}

/// Report containing all extracted messages.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MessagesReport {
    pub messages: Vec<MessageRecord>,
    pub stats: MessagesStats,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) pretty: bool,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) show_usage: bool,
}

/// Aggregate stats for the messages report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MessagesStats {
    pub total_messages: usize,
    pub total_sessions: usize,
    pub total_chars: usize,
    pub by_role: HashMap<String, usize>,
    /// Total input tokens across all assistant turns (when usage data available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_input_tokens: Option<u64>,
    /// Total output tokens across all assistant turns (when usage data available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_output_tokens: Option<u64>,
}

impl OutputFormatter for MessagesReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        for msg in &self.messages {
            let id_short = if msg.session_id.len() > 8 {
                &msg.session_id[..8]
            } else {
                &msg.session_id
            };
            let ts = msg.timestamp.as_deref().unwrap_or("?");
            // Trim timestamp to just date+time (no timezone)
            let ts_short = if ts.len() > 19 { &ts[..19] } else { ts };
            let ts_display = ts_short.replace('T', " ");

            let usage_suffix = if self.show_usage {
                format_usage_text(msg.usage.as_ref())
            } else {
                String::new()
            };

            lines.push(format!(
                "[{}] turn {} ({}, {}){} {}",
                id_short, msg.turn, msg.role, ts_display, usage_suffix, msg.text
            ));
        }

        // Summary line
        let role_summary: Vec<String> = {
            let mut pairs: Vec<_> = self.stats.by_role.iter().collect();
            pairs.sort_by_key(|(k, _)| (*k).clone());
            pairs.iter().map(|(k, v)| format!("{}: {}", k, v)).collect()
        };
        let token_summary = if self.show_usage {
            match (
                self.stats.total_input_tokens,
                self.stats.total_output_tokens,
            ) {
                (Some(i), Some(o)) => format!(" | tokens: in={} out={}", i, o),
                _ => String::new(),
            }
        } else {
            String::new()
        };
        lines.push(format!(
            "--- {} messages from {} sessions ({}){} ---",
            self.stats.total_messages,
            self.stats.total_sessions,
            role_summary.join(", "),
            token_summary,
        ));

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();

        for msg in &self.messages {
            let id_short = if msg.session_id.len() > 8 {
                &msg.session_id[..8]
            } else {
                &msg.session_id
            };
            let ts = msg.timestamp.as_deref().unwrap_or("?");
            let ts_short = if ts.len() > 19 { &ts[..19] } else { ts };
            let ts_display = ts_short.replace('T', " ");

            let role_badge = match msg.role.as_str() {
                "user" => "\x1b[34m[user]\x1b[0m",
                "assistant" => "\x1b[32m[asst]\x1b[0m",
                "system" => "\x1b[33m[sys]\x1b[0m",
                "tool" => "\x1b[35m[tool]\x1b[0m",
                _ => &msg.role,
            };

            let project_tag = msg
                .project
                .as_deref()
                .map(|p| format!(" \x1b[36m{}\x1b[0m", p))
                .unwrap_or_default();

            let usage_tag = if self.show_usage {
                format_usage_pretty(msg.usage.as_ref())
            } else {
                String::new()
            };

            lines.push(format!(
                "\x1b[33m{}\x1b[0m {} \x1b[90m{}\x1b[0m{}{} {}",
                id_short, role_badge, ts_display, project_tag, usage_tag, msg.text
            ));
        }

        // Summary
        let role_summary: Vec<String> = {
            let mut pairs: Vec<_> = self.stats.by_role.iter().collect();
            pairs.sort_by_key(|(k, _)| (*k).clone());
            pairs.iter().map(|(k, v)| format!("{}: {}", k, v)).collect()
        };
        let token_summary = if self.show_usage {
            match (
                self.stats.total_input_tokens,
                self.stats.total_output_tokens,
            ) {
                (Some(i), Some(o)) => format!(" | \x1b[33mtokens:\x1b[0m in={} out={}", i, o),
                _ => String::new(),
            }
        } else {
            String::new()
        };
        lines.push(format!(
            "\x1b[1m--- {} messages from {} sessions ({}){} ---\x1b[0m",
            self.stats.total_messages,
            self.stats.total_sessions,
            role_summary.join(", "),
            token_summary,
        ));

        lines.join("\n")
    }
}

/// Format usage as a compact text suffix: ` [in:1234 out:567]`
fn format_usage_text(usage: Option<&TokenUsage>) -> String {
    let Some(u) = usage else {
        return String::new();
    };
    let mut parts = vec![format!("in:{}", u.input), format!("out:{}", u.output)];
    if let Some(cr) = u.cache_read.filter(|&v| v > 0) {
        parts.push(format!("cache_read:{}", cr));
    }
    if let Some(cc) = u.cache_create.filter(|&v| v > 0) {
        parts.push(format!("cache_create:{}", cc));
    }
    format!(" [{}]", parts.join(" "))
}

/// Format usage as a colored pretty tag for terminal output.
fn format_usage_pretty(usage: Option<&TokenUsage>) -> String {
    let Some(u) = usage else {
        return String::new();
    };
    let mut parts = vec![
        format!("\x1b[33min:{}\x1b[0m", u.input),
        format!("\x1b[32mout:{}\x1b[0m", u.output),
    ];
    if let Some(cr) = u.cache_read.filter(|&v| v > 0) {
        parts.push(format!("\x1b[36mcr:{}\x1b[0m", cr));
    }
    if let Some(cc) = u.cache_create.filter(|&v| v > 0) {
        parts.push(format!("\x1b[35mcc:{}\x1b[0m", cc));
    }
    format!(" \x1b[90m[{}]\x1b[0m", parts.join(" "))
}

/// Build a messages report by extracting messages from sessions.
#[allow(clippy::too_many_arguments)]
pub fn build_messages_report(
    root: Option<&Path>,
    limit: usize,
    format_name: Option<&str>,
    role: RoleFilter,
    grep: Option<&str>,
    days: Option<u32>,
    since: Option<&str>,
    until: Option<&str>,
    project_filter: Option<&Path>,
    all_projects: bool,
    max_chars: Option<usize>,
    no_truncate: bool,
    show_usage: bool,
    sort_by_tokens: bool,
    pretty: bool,
) -> Result<MessagesReport, String> {
    let registry = FormatRegistry::new();
    let format: &dyn LogFormat = match format_name {
        Some(name) => registry
            .get(name)
            .ok_or_else(|| format!("Unknown format: {}", name))?,
        None => registry.get("claude").unwrap(),
    };

    let grep_re = grep
        .map(|p| regex::Regex::new(p).map_err(|_| format!("Invalid grep pattern: {}", p)))
        .transpose()?;

    let mut sessions: Vec<SessionFile> = if all_projects {
        list_all_project_sessions(format)
    } else {
        let proj = project_filter.or(root);
        format.list_sessions(proj)
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

    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    if limit > 0 {
        sessions.truncate(limit);
    }

    if sessions.is_empty() {
        return Err("No sessions found".to_string());
    }

    let role_filter = role;
    let max_text_len = if no_truncate {
        usize::MAX
    } else {
        max_chars.unwrap_or(200)
    };

    let mut messages = Vec::new();
    let mut session_count = 0;

    for sf in &sessions {
        let Ok(session) = format.parse(&sf.path) else {
            continue;
        };
        session_count += 1;

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
                // Role filter
                let role_str = msg.role.to_string();
                match role_filter {
                    RoleFilter::All => {}
                    RoleFilter::User => {
                        if role_str != "user" {
                            continue;
                        }
                    }
                    RoleFilter::Assistant => {
                        if role_str != "assistant" {
                            continue;
                        }
                    }
                }

                // Extract text from content blocks
                let text: String = msg
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                if text.is_empty() {
                    continue;
                }

                // Grep filter
                if let Some(ref re) = grep_re
                    && !re.is_match(&text)
                {
                    continue;
                }

                // Truncate + collapse whitespace for display
                let display_text = truncate_text(&text, max_text_len);

                // Token usage: attach to user messages (the trigger) and the first
                // assistant message. This way --role user (default) shows cost-per-prompt,
                // and --role assistant shows cost-per-response.
                let usage = if role_str == "user" || role_str == "assistant" {
                    turn.token_usage.clone()
                } else {
                    None
                };

                messages.push(MessageRecord {
                    session_id: session_id.clone(),
                    project: project.clone(),
                    turn: turn_idx,
                    role: role_str,
                    timestamp: msg.timestamp.clone(),
                    char_count: text.len(),
                    text: display_text,
                    usage,
                });
            }
        }
    }

    // Sort by descending total tokens if requested
    if sort_by_tokens {
        messages.sort_by(|a, b| {
            let tok_a = a.usage.as_ref().map(|u| u.input + u.output).unwrap_or(0);
            let tok_b = b.usage.as_ref().map(|u| u.input + u.output).unwrap_or(0);
            tok_b.cmp(&tok_a)
        });
    }

    // Build stats
    let mut by_role: HashMap<String, usize> = HashMap::new();
    let mut total_chars = 0;
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut has_token_data = false;
    for msg in &messages {
        *by_role.entry(msg.role.clone()).or_insert(0) += 1;
        total_chars += msg.char_count;
        if let Some(ref u) = msg.usage {
            total_input_tokens += u.input;
            total_output_tokens += u.output;
            has_token_data = true;
        }
    }

    let stats = MessagesStats {
        total_messages: messages.len(),
        total_sessions: session_count,
        total_chars,
        by_role,
        total_input_tokens: has_token_data.then_some(total_input_tokens),
        total_output_tokens: has_token_data.then_some(total_output_tokens),
    };

    Ok(MessagesReport {
        messages,
        stats,
        pretty,
        show_usage,
    })
}

/// Truncate text to max_len characters without collapsing whitespace.
fn truncate_text(s: &str, max_len: usize) -> String {
    let trimmed = s.trim();
    if trimmed.len() <= max_len {
        return trimmed.to_string();
    }
    let mut end = max_len;
    // Don't cut in the middle of a multi-byte char
    while !trimmed.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    format!("{}...", &trimmed[..end])
}
