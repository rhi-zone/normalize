//! Shared parser for Anthropic `api_conversation_history.json` format.
//!
//! Both Cline and Roo-Code store conversation history as a JSON array of
//! `Anthropic.MessageParam` objects (optionally extended with extra fields
//! like `ts`, `isSummary`, `reasoning_content` in the Roo-Code variant).
//! This module contains the common parsing logic used by both formats.

use super::{DiscoverError, ParseError, SessionLocation, SessionRef, read_file};
use crate::{ContentBlock, Message, Role, Session, Turn};
use serde_json::Value;
use std::path::Path;
use std::time::SystemTime;

/// Parse a `tasks/<taskId>/api_conversation_history.json` file into a `Session`.
///
/// `task_dir` is the directory containing `api_conversation_history.json`.
/// `format_name` is the format identifier set on the returned `Session`.
/// `task_id` is used as the session metadata `session_id`.
pub(crate) fn load_from_task_dir(
    task_dir: &Path,
    format_name: &str,
    task_id: &str,
) -> Result<Session, ParseError> {
    let history_path = task_dir.join("api_conversation_history.json");
    let content = read_file(&history_path)?;

    let entries: Vec<Value> = serde_json::from_str(&content).map_err(|e| ParseError::Format {
        path: history_path.clone(),
        message: format!("invalid JSON array: {e}"),
    })?;

    let mut session = Session::new(task_dir.to_path_buf(), format_name);
    session.metadata.session_id = Some(task_id.to_string());
    session.metadata.provider = Some("anthropic".to_string());
    session.subagent_type = Some("interactive".into());

    let mut current_turn = Turn::default();

    for entry in &entries {
        // Skip roo-code-specific summary/truncation markers
        if entry
            .get("isSummary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        if entry
            .get("isTruncationMarker")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }

        let role_str = entry.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let role = match role_str {
            "assistant" => Role::Assistant,
            _ => Role::User,
        };

        // Timestamp: roo-code stores it as `ts` (ms epoch)
        let timestamp = entry.get("ts").and_then(|v| v.as_u64()).map(format_ts_ms);

        let message = parse_anthropic_message(entry, role, timestamp);

        // Tool-result-only user messages stay in the current turn as Tool role
        let is_tool_result_only = role == Role::User
            && !message.content.is_empty()
            && message
                .content
                .iter()
                .all(|b| matches!(b, ContentBlock::ToolResult { .. }));

        if is_tool_result_only {
            let mut tool_msg = message;
            tool_msg.role = Role::Tool;
            current_turn.messages.push(tool_msg);
        } else if role == Role::User {
            // New human prompt — flush previous turn
            if !current_turn.messages.is_empty() {
                session.turns.push(std::mem::take(&mut current_turn));
            }
            current_turn.messages.push(message);
        } else {
            // Assistant message — belongs to current turn
            current_turn.messages.push(message);
        }
    }

    if !current_turn.messages.is_empty() {
        session.turns.push(current_turn);
    }

    Ok(session)
}

/// Parse one Anthropic `MessageParam` (or roo-code `ApiMessage`) into a `Message`.
fn parse_anthropic_message(entry: &Value, role: Role, timestamp: Option<String>) -> Message {
    let mut blocks = Vec::new();

    // roo-code: DeepSeek/Z.ai interleaved thinking field
    if let Some(reasoning) = entry
        .get("reasoning_content")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        blocks.push(ContentBlock::Thinking {
            text: reasoning.to_string(),
        });
    }

    let content = entry.get("content");
    match content {
        Some(Value::String(s)) if !s.is_empty() => {
            blocks.push(ContentBlock::Text { text: s.clone() });
        }
        Some(Value::Array(arr)) => {
            for block in arr {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str())
                            && !text.is_empty()
                        {
                            blocks.push(ContentBlock::Text {
                                text: text.to_string(),
                            });
                        }
                    }
                    "tool_use" => {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let input = block.get("input").cloned().unwrap_or(Value::Null);
                        blocks.push(ContentBlock::ToolUse { id, name, input });
                    }
                    "tool_result" => {
                        let tool_use_id = block
                            .get("tool_use_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let result_content = match block.get("content") {
                            Some(Value::String(s)) => s.clone(),
                            Some(Value::Array(arr)) => arr
                                .iter()
                                .filter_map(|b| {
                                    b.get("text").and_then(|t| t.as_str()).map(String::from)
                                })
                                .collect::<Vec<_>>()
                                .join("\n"),
                            _ => String::new(),
                        };
                        let is_error = block
                            .get("is_error")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        blocks.push(ContentBlock::ToolResult {
                            tool_use_id,
                            content: result_content,
                            is_error,
                        });
                    }
                    "thinking" => {
                        if let Some(text) = block.get("thinking").and_then(|v| v.as_str()) {
                            blocks.push(ContentBlock::Thinking {
                                text: text.to_string(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    Message {
        role,
        content: blocks,
        timestamp,
    }
}

/// Format a Unix timestamp in milliseconds as an ISO 8601 UTC string.
fn format_ts_ms(ms: u64) -> String {
    let secs = ms / 1000;
    let (y, mo, d, h, mi, sec) = secs_to_ymdhms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{sec:02}Z")
}

/// Decompose Unix seconds into `(year, month, day, hour, min, sec)` UTC.
// normalize-syntax-allow: rust/tuple-return - private calendar arithmetic; named struct would add noise with no benefit at this call-site count
fn secs_to_ymdhms(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = secs % 60;
    let min = (secs / 60) % 60;
    let hour = (secs / 3600) % 24;
    let mut days = secs / 86400;

    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days: &[u64] = if is_leap(year) {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &md in month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1, hour, min, sec)
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// Walk `root/tasks/` and return a `SessionRef` for each subdir that has
/// `api_conversation_history.json`.
///
/// Shared by `ClineFormat` and `RooCodeFormat`.
pub(crate) fn discover_task_dirs(
    root: &Path,
    format_name: &'static str,
) -> Result<Vec<SessionRef>, DiscoverError> {
    let tasks_dir = root.join("tasks");
    let mut sessions = Vec::new();

    let Ok(entries) = std::fs::read_dir(&tasks_dir) else {
        // tasks/ doesn't exist — not an error, just no sessions.
        return Ok(sessions);
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let task_dir = entry.path();
        if !task_dir.is_dir() {
            continue;
        }
        let history_file = task_dir.join("api_conversation_history.json");
        if !history_file.exists() {
            continue;
        }
        // Use the history file's mtime (reflects last write) rather than the
        // directory's mtime (reflects entry add/remove only).
        let mtime = history_file
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        sessions.push(SessionRef {
            format: format_name,
            location: SessionLocation::Directory(task_dir.clone()),
            path: task_dir,
            mtime,
            parent_session_id: None,
            agent_id: None,
            subagent_type: Some("interactive".into()),
        });
    }

    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_ts_ms_known_value() {
        // 2024-01-15T12:30:00Z = 1705321800 seconds
        // Verify: 19737 days * 86400 + (12*3600 + 30*60) = 1705276800 + 45000 = 1705321800
        let ms = 1705321800u64 * 1000;
        assert_eq!(format_ts_ms(ms), "2024-01-15T12:30:00Z");
    }
}
