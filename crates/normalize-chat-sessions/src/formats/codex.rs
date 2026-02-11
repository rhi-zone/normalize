//! OpenAI Codex CLI JSONL format parser.

use super::{LogFormat, SessionFile, peek_lines};
use crate::{ContentBlock, Message, Role, Session, TokenUsage, Turn};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// OpenAI Codex CLI session log format (JSONL).
pub struct CodexFormat;

impl LogFormat for CodexFormat {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn sessions_dir(&self, _project: Option<&Path>) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".codex/sessions")
    }

    fn list_sessions(&self, project: Option<&Path>) -> Vec<SessionFile> {
        let dir = self.sessions_dir(project);
        // Codex stores sessions in ~/.codex/sessions/YYYY/MM/DD/*.jsonl
        let mut sessions = Vec::new();
        // Walk year directories
        if let Ok(years) = std::fs::read_dir(&dir) {
            for year in years.filter_map(|e| e.ok()) {
                if !year.path().is_dir() {
                    continue;
                }
                // Walk month directories
                if let Ok(months) = std::fs::read_dir(year.path()) {
                    for month in months.filter_map(|e| e.ok()) {
                        if !month.path().is_dir() {
                            continue;
                        }
                        // Walk day directories
                        if let Ok(days) = std::fs::read_dir(month.path()) {
                            for day in days.filter_map(|e| e.ok()) {
                                if !day.path().is_dir() {
                                    continue;
                                }
                                // Find .jsonl files
                                if let Ok(files) = std::fs::read_dir(day.path()) {
                                    for file in files.filter_map(|e| e.ok()) {
                                        let path = file.path();
                                        if path.extension().and_then(|e| e.to_str())
                                            == Some("jsonl")
                                            && let Ok(meta) = path.metadata()
                                            && let Ok(mtime) = meta.modified()
                                        {
                                            sessions.push(SessionFile { path, mtime });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        sessions
    }

    fn detect(&self, path: &Path) -> f64 {
        // Check extension
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "jsonl" {
            return 0.0;
        }

        // Peek at first few lines
        for line in peek_lines(path, 5) {
            if let Ok(entry) = serde_json::from_str::<Value>(&line) {
                // Codex has type field with session_meta, response_item, event_msg
                if let Some(t) = entry.get("type").and_then(|v| v.as_str())
                    && t == "session_meta"
                {
                    // Check for codex-specific originator
                    if let Some(originator) = entry
                        .get("payload")
                        .and_then(|p| p.get("originator"))
                        .and_then(|v| v.as_str())
                        && originator.contains("codex")
                    {
                        return 1.0;
                    }
                }
            }
        }
        0.0
    }

    fn parse(&self, path: &Path) -> Result<Session, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);

        let mut session = Session::new(path.to_path_buf(), self.name());
        let mut current_turn = Turn::default();
        let mut pending_tool_calls: HashMap<String, (String, Value)> = HashMap::new();

        for line in reader.lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.trim().is_empty() {
                continue;
            }

            let Ok(entry) = serde_json::from_str::<Value>(&line) else {
                continue;
            };

            let entry_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");

            // Extract metadata from session_meta
            if entry_type == "session_meta"
                && let Some(payload) = entry.get("payload")
            {
                if session.metadata.session_id.is_none() {
                    session.metadata.session_id = payload
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }
                if session.metadata.model.is_none() {
                    session.metadata.model = payload
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }
            }

            let Some(payload) = entry.get("payload") else {
                continue;
            };

            let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match payload_type {
                "user_message" => {
                    // Flush previous turn
                    if !current_turn.messages.is_empty() {
                        session.turns.push(std::mem::take(&mut current_turn));
                    }

                    let text = payload
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    current_turn.messages.push(Message {
                        role: Role::User,
                        content: vec![ContentBlock::Text { text }],
                        timestamp: entry
                            .get("timestamp")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
                "message" => {
                    // Assistant text response
                    let text = payload
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !text.is_empty() {
                        current_turn.messages.push(Message {
                            role: Role::Assistant,
                            content: vec![ContentBlock::Text { text }],
                            timestamp: entry
                                .get("timestamp")
                                .and_then(|v| v.as_str())
                                .map(String::from),
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
                    let args_str = payload
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    let input: Value =
                        serde_json::from_str(args_str).unwrap_or(Value::Object(Default::default()));

                    // Store for later pairing with result
                    pending_tool_calls.insert(call_id.clone(), (name.clone(), input.clone()));

                    current_turn.messages.push(Message {
                        role: Role::Assistant,
                        content: vec![ContentBlock::ToolUse {
                            id: call_id,
                            name,
                            input,
                        }],
                        timestamp: entry
                            .get("timestamp")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
                "function_call_output" => {
                    let call_id = payload
                        .get("call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let output = payload
                        .get("output")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let is_error = output.contains("Exit code: 1")
                        || output.starts_with("Error:")
                        || output.contains("\nError:");

                    current_turn.messages.push(Message {
                        role: Role::User,
                        content: vec![ContentBlock::ToolResult {
                            tool_use_id: call_id,
                            content: output,
                            is_error,
                        }],
                        timestamp: entry
                            .get("timestamp")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
                "token_count" => {
                    // Extract final token usage
                    if let Some(info) = payload.get("info")
                        && let Some(total) = info.get("total_token_usage")
                    {
                        current_turn.token_usage = Some(TokenUsage {
                            input: total
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            output: total
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                + total
                                    .get("reasoning_output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0),
                            cache_read: total.get("cached_input_tokens").and_then(|v| v.as_u64()),
                            cache_create: None,
                            model: session.metadata.model.clone(),
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

        // Set provider
        session.metadata.provider = Some("openai".to_string());

        Ok(session)
    }
}
