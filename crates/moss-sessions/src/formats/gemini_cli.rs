//! Gemini CLI JSON format parser.

use super::{LogFormat, SessionFile, read_file};
use crate::{ContentBlock, Message, Role, Session, TokenUsage, Turn};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Gemini CLI session log format (JSON with messages array).
pub struct GeminiCliFormat;

impl LogFormat for GeminiCliFormat {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn sessions_dir(&self, _project: Option<&Path>) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".gemini/tmp")
    }

    fn list_sessions(&self, project: Option<&Path>) -> Vec<SessionFile> {
        let dir = self.sessions_dir(project);
        // Gemini stores sessions in ~/.gemini/tmp/<hash>/logs.json
        let mut sessions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let subdir = entry.path();
                if !subdir.is_dir() {
                    continue;
                }
                let logs_path = subdir.join("logs.json");
                if logs_path.exists() {
                    if let Ok(meta) = logs_path.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            sessions.push(SessionFile {
                                path: logs_path,
                                mtime,
                            });
                        }
                    }
                }
            }
        }
        sessions
    }

    fn detect(&self, path: &Path) -> f64 {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "json" {
            return 0.0;
        }

        // Try to parse as JSON (not JSONL)
        let Ok(content) = read_file(path) else {
            return 0.0;
        };

        let Ok(data) = serde_json::from_str::<Value>(&content) else {
            return 0.0;
        };

        // Gemini CLI has sessionId and messages array with type="gemini"
        if data.get("sessionId").is_some() && data.get("messages").is_some() {
            if let Some(messages) = data.get("messages").and_then(|m| m.as_array()) {
                for msg in messages {
                    if msg.get("type").and_then(|t| t.as_str()) == Some("gemini") {
                        return 1.0;
                    }
                }
            }
            return 0.5; // Has structure but no gemini messages yet
        }

        0.0
    }

    fn parse(&self, path: &Path) -> Result<Session, String> {
        let content = read_file(path)?;
        let data: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;

        let mut session = Session::new(path.to_path_buf(), self.name());

        // Extract metadata
        session.metadata.session_id = data
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(String::from);
        session.metadata.provider = Some("google".to_string());

        let messages = data
            .get("messages")
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default();

        let mut current_turn = Turn::default();

        for msg in &messages {
            let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match msg_type {
                "user" => {
                    // Flush previous turn
                    if !current_turn.messages.is_empty() {
                        session.turns.push(std::mem::take(&mut current_turn));
                    }

                    let message = parse_user_message(msg);
                    current_turn.messages.push(message);
                }
                "gemini" => {
                    // Extract model from first gemini message
                    if session.metadata.model.is_none() {
                        session.metadata.model =
                            msg.get("model").and_then(|v| v.as_str()).map(String::from);
                    }

                    let message = parse_gemini_message(msg);
                    current_turn.messages.push(message);

                    // Extract token usage
                    if let Some(tokens) = msg.get("tokens") {
                        current_turn.token_usage = Some(TokenUsage {
                            input: tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0),
                            output: tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0),
                            cache_read: tokens.get("cached").and_then(|v| v.as_u64()),
                            cache_create: None,
                        });
                    }
                }
                _ => {}
            }
        }

        // Flush final turn
        if !current_turn.messages.is_empty() {
            session.turns.push(current_turn);
        }

        Ok(session)
    }
}

/// Parse a user message from Gemini CLI format.
fn parse_user_message(msg: &Value) -> Message {
    let mut content = Vec::new();

    if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
        content.push(ContentBlock::Text {
            text: text.to_string(),
        });
    }

    Message {
        role: Role::User,
        content,
        timestamp: msg
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from),
    }
}

/// Parse a gemini (assistant) message from Gemini CLI format.
fn parse_gemini_message(msg: &Value) -> Message {
    let mut content = Vec::new();

    // Text content
    if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
        content.push(ContentBlock::Text {
            text: text.to_string(),
        });
    }

    // Tool calls
    if let Some(tool_calls) = msg.get("toolCalls").and_then(|t| t.as_array()) {
        for tc in tool_calls {
            let id = tc
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = tc
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input = tc.get("args").cloned().unwrap_or(Value::Null);

            content.push(ContentBlock::ToolUse {
                id: id.clone(),
                name,
                input,
            });

            // Tool result (Gemini includes result in the same message)
            if let Some(result) = tc.get("result") {
                let tool_use_id = id;
                let result_content = if let Some(s) = result.as_str() {
                    s.to_string()
                } else {
                    result.to_string()
                };
                let is_error = tc.get("status").and_then(|s| s.as_str()) == Some("error");
                content.push(ContentBlock::ToolResult {
                    tool_use_id,
                    content: result_content,
                    is_error,
                });
            }
        }
    }

    Message {
        role: Role::Assistant,
        content,
        timestamp: msg
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from),
    }
}
