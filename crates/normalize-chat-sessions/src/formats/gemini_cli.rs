//! Gemini CLI JSON format parser.
//!
//! TODO(phase2): rewrite for current Gemini CLI format (session layout may have changed).

use super::{DiscoverError, ParseError, SessionLocation, SessionRef, SessionSource, read_file};
use crate::{ContentBlock, Message, Role, Session, TokenUsage, Turn};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Gemini CLI session log format (JSON with messages array).
pub struct GeminiCliFormat;

impl SessionSource for GeminiCliFormat {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn sessions_root(&self, _project: Option<&Path>) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".gemini/tmp")
    }

    fn detect(&self, path: &Path) -> f64 {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "json" {
            return 0.0;
        }

        let Ok(content) = read_file(path) else {
            return 0.0;
        };

        let Ok(data) = serde_json::from_str::<Value>(&content) else {
            return 0.0;
        };

        if data.get("sessionId").is_some() && data.get("messages").is_some() {
            if let Some(messages) = data.get("messages").and_then(|m| m.as_array()) {
                for msg in messages {
                    if msg.get("type").and_then(|t| t.as_str()) == Some("gemini") {
                        return 1.0;
                    }
                }
            }
            return 0.5;
        }

        0.0
    }

    fn discover(&self, root: &Path) -> Result<Vec<SessionRef>, DiscoverError> {
        // Gemini stores sessions in ~/.gemini/tmp/<hash>/logs.json
        let mut refs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.filter_map(|e| e.ok()) {
                let subdir = entry.path();
                if !subdir.is_dir() {
                    continue;
                }
                let logs_path = subdir.join("logs.json");
                if logs_path.exists()
                    && let Ok(meta) = logs_path.metadata()
                    && let Ok(mtime) = meta.modified()
                {
                    refs.push(SessionRef {
                        format: self.name(),
                        location: SessionLocation::File(logs_path.clone()),
                        path: logs_path,
                        mtime,
                        parent_session_id: None,
                        agent_id: None,
                        subagent_type: Some("interactive".into()),
                    });
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
        self.parse_path(path)
    }
}

impl GeminiCliFormat {
    fn parse_path(&self, path: &Path) -> Result<Session, ParseError> {
        let content = read_file(path)?;
        let data: Value = serde_json::from_str(&content).map_err(|e| ParseError::Format {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        let mut session = Session::new(path.to_path_buf(), self.name());
        session.subagent_type = Some("interactive".into());

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
                    if !current_turn.messages.is_empty() {
                        session.turns.push(std::mem::take(&mut current_turn));
                    }
                    let message = parse_user_message(msg);
                    current_turn.messages.push(message);
                }
                "gemini" => {
                    if session.metadata.model.is_none() {
                        session.metadata.model =
                            msg.get("model").and_then(|v| v.as_str()).map(String::from);
                    }

                    let message = parse_gemini_message(msg);
                    current_turn.messages.push(message);

                    if let Some(tokens) = msg.get("tokens") {
                        current_turn.token_usage = Some(TokenUsage {
                            input: tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0),
                            output: tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0),
                            cache_read: tokens.get("cached").and_then(|v| v.as_u64()),
                            cache_create: None,
                            model: msg.get("model").and_then(|v| v.as_str()).map(String::from),
                        });
                    }
                }
                _ => {}
            }
        }

        if !current_turn.messages.is_empty() {
            session.turns.push(current_turn);
        }

        Ok(session)
    }
}

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

fn parse_gemini_message(msg: &Value) -> Message {
    let mut content = Vec::new();

    if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
        content.push(ContentBlock::Text {
            text: text.to_string(),
        });
    }

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

            if let Some(result) = tc.get("result") {
                let result_content = if let Some(s) = result.as_str() {
                    s.to_string()
                } else {
                    result.to_string()
                };
                let is_error = tc.get("status").and_then(|s| s.as_str()) == Some("error");
                content.push(ContentBlock::ToolResult {
                    tool_use_id: id,
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
