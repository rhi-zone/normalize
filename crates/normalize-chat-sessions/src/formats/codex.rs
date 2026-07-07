//! OpenAI Codex CLI JSONL rollout format parser.
//!
//! Codex records sessions as JSONL "rollout" files under:
//!   `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-{date}-{thread_id}.jsonl`
//! (default `CODEX_HOME` = `~/.codex`).
//!
//! Each line is a `RolloutLine`:
//!   `{"timestamp": "...", "type": "<item_type>", "payload": {...}}`
//!
//! The first line is always `type=session_meta`. Subsequent lines are typically
//! `type=response_item` containing a Responses-API `ResponseItem`.
//!
//! Reference: `codex-rs/protocol/src/protocol.rs` + `codex-rs/rollout/src/recorder.rs`.

use super::{DiscoverError, ParseError, SessionLocation, SessionRef, SessionSource, peek_lines};
use crate::{ContentBlock, Message, Role, Session, Turn};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Codex CLI session format (rollout JSONL).
pub struct CodexFormat;

impl SessionSource for CodexFormat {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn sessions_root(&self, _project: Option<&Path>) -> PathBuf {
        let home = std::env::var("CODEX_HOME").unwrap_or_else(|_| {
            let h = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            format!("{h}/.codex")
        });
        PathBuf::from(home).join("sessions")
    }

    /// Returns 1.0 if the first line of `path` is a `session_meta` RolloutLine.
    fn detect(&self, path: &Path) -> f64 {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "jsonl" {
            return 0.0;
        }
        for line in peek_lines(path, 1) {
            if let Ok(entry) = serde_json::from_str::<Value>(&line)
                && entry.get("type").and_then(|v| v.as_str()) == Some("session_meta")
                && entry
                    .get("payload")
                    .and_then(|p| p.get("session_id"))
                    .is_some()
            {
                return 1.0;
            }
        }
        0.0
    }

    /// Walk `root/YYYY/MM/DD/rollout-*.jsonl`, reading the first line of each
    /// to extract `session_id` and `parent_thread_id`.
    fn discover(&self, root: &Path) -> Result<Vec<SessionRef>, DiscoverError> {
        let mut refs = Vec::new();
        let Ok(years) = std::fs::read_dir(root) else {
            return Ok(refs);
        };
        for year in years.filter_map(|e| e.ok()) {
            if !year.path().is_dir() {
                continue;
            }
            let Ok(months) = std::fs::read_dir(year.path()) else {
                continue;
            };
            for month in months.filter_map(|e| e.ok()) {
                if !month.path().is_dir() {
                    continue;
                }
                let Ok(days) = std::fs::read_dir(month.path()) else {
                    continue;
                };
                for day in days.filter_map(|e| e.ok()) {
                    if !day.path().is_dir() {
                        continue;
                    }
                    let Ok(files) = std::fs::read_dir(day.path()) else {
                        continue;
                    };
                    for file in files.filter_map(|e| e.ok()) {
                        let path = file.path();
                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        if ext != "jsonl" {
                            continue;
                        }
                        let Ok(meta) = path.metadata() else {
                            continue;
                        };
                        let Ok(mtime) = meta.modified() else {
                            continue;
                        };
                        let meta = read_session_meta_from_rollout(&path);
                        refs.push(SessionRef {
                            format: self.name(),
                            location: SessionLocation::File(path.clone()),
                            path,
                            mtime,
                            parent_session_id: meta.parent_thread_id,
                            agent_id: meta.session_id,
                            subagent_type: Some("interactive".into()),
                        });
                    }
                }
            }
        }
        Ok(refs)
    }

    fn load(&self, r: &SessionRef) -> Result<Session, ParseError> {
        let path = match &r.location {
            SessionLocation::File(p) => p.as_path(),
            _ => &r.path,
        };
        parse_rollout(path)
    }
}

/// Metadata extracted from the first line of a rollout JSONL file.
struct RolloutMeta {
    session_id: Option<String>,
    parent_thread_id: Option<String>,
}

/// Read the first line of a rollout file and return its session metadata.
fn read_session_meta_from_rollout(path: &Path) -> RolloutMeta {
    for line in peek_lines(path, 1) {
        if let Ok(entry) = serde_json::from_str::<Value>(&line)
            && entry.get("type").and_then(|v| v.as_str()) == Some("session_meta")
        {
            let payload = entry.get("payload");
            return RolloutMeta {
                session_id: payload
                    .and_then(|p| p.get("session_id"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                parent_thread_id: payload
                    .and_then(|p| p.get("parent_thread_id"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
            };
        }
    }
    RolloutMeta {
        session_id: None,
        parent_thread_id: None,
    }
}

/// Parse a `rollout-*.jsonl` file into a `Session`.
///
/// Line structure:
/// ```json
/// {"timestamp":"...","type":"session_meta","payload":{...SessionMeta...}}
/// {"timestamp":"...","type":"response_item","payload":{...ResponseItem...}}
/// ```
///
/// ResponseItem variants we map:
/// - `message` (role=user) → new Turn, User Message
/// - `message` (role=assistant) → Assistant Message, Text blocks from `content`
/// - `reasoning` → Thinking block from `summary[].text`
/// - `function_call` → ToolUse (name, call_id, arguments as JSON)
/// - `function_call_output` → ToolResult (call_id, output string or items)
fn parse_rollout(path: &Path) -> Result<Session, ParseError> {
    let file = File::open(path).map_err(|e| ParseError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let reader = BufReader::new(file);

    let mut session = Session::new(path.to_path_buf(), "codex");
    let mut current_turn = Turn::default();

    for raw in reader.lines() {
        let raw = raw.map_err(|e| ParseError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        if raw.trim().is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };

        let entry_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = entry
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from);

        match entry_type {
            "session_meta" => {
                let payload = entry.get("payload");
                if session.metadata.session_id.is_none() {
                    session.metadata.session_id = payload
                        .and_then(|p| p.get("session_id"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }
                if session.metadata.timestamp.is_none() {
                    session.metadata.timestamp = timestamp.clone().or_else(|| {
                        payload
                            .and_then(|p| p.get("timestamp"))
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    });
                }
                // model_provider is on SessionMeta; model name is not stored per-session
                if session.metadata.provider.is_none() {
                    session.metadata.provider = payload
                        .and_then(|p| p.get("model_provider"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .or_else(|| Some("openai".to_string()));
                }
                // parent_thread_id → parent_id (subagent link)
                if session.parent_id.is_none() {
                    session.parent_id = payload
                        .and_then(|p| p.get("parent_thread_id"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }
            }

            "response_item" => {
                let Some(payload) = entry.get("payload") else {
                    continue;
                };
                let item_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match item_type {
                    "message" => {
                        let role = payload
                            .get("role")
                            .and_then(|v| v.as_str())
                            .unwrap_or("assistant");
                        let content_blocks = extract_content_items(payload);

                        if role == "user" {
                            // User message → flush previous turn and start new one.
                            if !current_turn.messages.is_empty() {
                                session.turns.push(std::mem::take(&mut current_turn));
                            }
                            current_turn.messages.push(Message {
                                role: Role::User,
                                content: content_blocks,
                                timestamp,
                            });
                        } else {
                            // assistant (or system) message
                            current_turn.messages.push(Message {
                                role: Role::Assistant,
                                content: content_blocks,
                                timestamp,
                            });
                        }
                    }

                    "reasoning" => {
                        // Collect reasoning summary text as a Thinking block.
                        let text = payload
                            .get("summary")
                            .and_then(|s| s.as_array())
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default();

                        if !text.is_empty() {
                            current_turn.messages.push(Message {
                                role: Role::Assistant,
                                content: vec![ContentBlock::Thinking { text }],
                                timestamp,
                            });
                        }
                    }

                    "function_call" => {
                        let call_id = payload
                            .get("call_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = payload
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        // `arguments` is a JSON string on the wire; parse it.
                        let input = payload
                            .get("arguments")
                            .and_then(|v| v.as_str())
                            .and_then(|s| serde_json::from_str::<Value>(s).ok())
                            .unwrap_or(Value::Object(Default::default()));

                        current_turn.messages.push(Message {
                            role: Role::Assistant,
                            content: vec![ContentBlock::ToolUse {
                                id: call_id,
                                name,
                                input,
                            }],
                            timestamp,
                        });
                    }

                    "function_call_output" => {
                        let call_id = payload
                            .get("call_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        // `output` serializes as either a plain string or an array of
                        // content items (FunctionCallOutputPayload custom serde).
                        let output_val = payload.get("output");
                        let output_text = match output_val {
                            Some(Value::String(s)) => s.clone(),
                            Some(Value::Array(items)) => items
                                .iter()
                                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n"),
                            _ => String::new(),
                        };
                        let is_error = payload
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .map(|ok| !ok)
                            .unwrap_or(false);

                        current_turn.messages.push(Message {
                            role: Role::User,
                            content: vec![ContentBlock::ToolResult {
                                tool_use_id: call_id,
                                content: output_text,
                                is_error,
                            }],
                            timestamp,
                        });
                    }

                    _ => {
                        // local_shell_call, compacted, agent_message, etc. — skip.
                    }
                }
            }

            // session_meta, compacted, turn_context, world_state, event_msg — skip
            _ => {}
        }
    }

    if !current_turn.messages.is_empty() {
        session.turns.push(current_turn);
    }

    Ok(session)
}

/// Extract text `ContentBlock`s from a `ResponseItem::Message` payload's `content` array.
///
/// ContentItem wire format: `{"type": "input_text"|"output_text", "text": "..."}`.
fn extract_content_items(payload: &Value) -> Vec<ContentBlock> {
    let Some(arr) = payload.get("content").and_then(|c| c.as_array()) else {
        return Vec::new();
    };
    let mut blocks = Vec::new();
    for item in arr {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match item_type {
            "input_text" | "output_text" => {
                if let Some(text) = item.get("text").and_then(|v| v.as_str())
                    && !text.is_empty()
                {
                    blocks.push(ContentBlock::Text {
                        text: text.to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    blocks
}

// Keep SystemTime in scope for SessionRef construction in discover()
const _: () = {
    let _ = SystemTime::UNIX_EPOCH;
};
