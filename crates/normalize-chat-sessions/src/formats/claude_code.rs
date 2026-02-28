//! Claude Code JSONL format parser.

use super::{LogFormat, SessionFile, list_jsonl_sessions, peek_lines};
use crate::{ContentBlock, Message, Role, Session, TokenUsage, Turn};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Claude Code session log format (JSONL).
pub struct ClaudeCodeFormat;

impl LogFormat for ClaudeCodeFormat {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn sessions_dir(&self, project: Option<&Path>) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let claude_dir = PathBuf::from(home).join(".claude/projects");

        // Claude encodes project paths - check which encoding variant exists
        let path_to_claude_dir = |path: &Path| -> PathBuf {
            let path_str = path.to_string_lossy().replace('/', "-");
            // Try with leading dash first (Claude's format)
            let proj_dir = claude_dir.join(format!("-{}", path_str.trim_start_matches('-')));
            if proj_dir.exists() {
                return proj_dir;
            }
            // Try without leading dash
            let proj_dir = claude_dir.join(&path_str);
            if proj_dir.exists() {
                return proj_dir;
            }
            // Return primary format even if it doesn't exist yet
            claude_dir.join(format!("-{}", path_str.trim_start_matches('-')))
        };

        if let Some(proj) = project {
            return path_to_claude_dir(proj);
        }

        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            && output.status.success()
        {
            return path_to_claude_dir(Path::new(String::from_utf8_lossy(&output.stdout).trim()));
        }

        if let Ok(cwd) = std::env::current_dir() {
            return path_to_claude_dir(&cwd);
        }

        claude_dir
    }

    fn list_sessions(&self, project: Option<&Path>) -> Vec<SessionFile> {
        list_jsonl_sessions(&self.sessions_dir(project))
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
                // Claude Code has type field with specific values
                if let Some(t) = entry.get("type").and_then(|v| v.as_str())
                    && matches!(
                        t,
                        "user" | "assistant" | "summary" | "file-history-snapshot"
                    )
                {
                    return 1.0;
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
        let mut request_tokens: HashMap<String, TokenUsage> = HashMap::new();
        let mut last_request_id: Option<String> = None;

        for line in reader.lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.trim().is_empty() {
                continue;
            }

            let Ok(entry) = serde_json::from_str::<Value>(&line) else {
                continue;
            };

            let Some(entry_type) = entry.get("type").and_then(|v| v.as_str()) else {
                continue;
            };

            match entry_type {
                "user" => {
                    let message = parse_message(&entry, Role::User);
                    // Tool result messages are structurally "user" in Claude Code's format
                    // but semantically they are tool responses, not human input.
                    let is_tool_result = !message.content.is_empty()
                        && message
                            .content
                            .iter()
                            .all(|b| matches!(b, ContentBlock::ToolResult { .. }));

                    if is_tool_result {
                        // Tool results belong to the current turn, not a new one
                        let mut tool_msg = message;
                        tool_msg.role = Role::Tool;
                        current_turn.messages.push(tool_msg);
                    } else {
                        // Flush previous turn if we have messages
                        if !current_turn.messages.is_empty() {
                            if let Some(req_id) = &last_request_id
                                && let Some(usage) = request_tokens.remove(req_id)
                            {
                                current_turn.token_usage = Some(usage);
                            }
                            session.turns.push(std::mem::take(&mut current_turn));
                        }
                        current_turn.messages.push(message);
                    }
                }
                "assistant" => {
                    let request_id = entry
                        .get("requestId")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    // Extract per-turn model
                    let turn_model = entry
                        .get("message")
                        .and_then(|m| m.get("model"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    // Extract token usage (take max per request due to streaming)
                    if let Some(usage) = entry.get("message").and_then(|m| m.get("usage")) {
                        let tokens = TokenUsage {
                            input: usage
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            output: usage
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            cache_read: usage
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_u64()),
                            cache_create: usage
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_u64()),
                            model: turn_model.clone(),
                        };
                        if let Some(ref req_id) = request_id {
                            let existing = request_tokens.entry(req_id.clone()).or_default();
                            existing.input = existing.input.max(tokens.input);
                            existing.output = existing.output.max(tokens.output);
                            if let Some(cr) = tokens.cache_read {
                                *existing.cache_read.get_or_insert(0) =
                                    existing.cache_read.unwrap_or(0).max(cr);
                            }
                            if let Some(cc) = tokens.cache_create {
                                *existing.cache_create.get_or_insert(0) =
                                    existing.cache_create.unwrap_or(0).max(cc);
                            }
                            if tokens.model.is_some() {
                                existing.model = tokens.model;
                            }
                        }
                    }

                    // Extract model from first assistant message
                    if session.metadata.model.is_none() {
                        session.metadata.model = entry
                            .get("message")
                            .and_then(|m| m.get("model"))
                            .and_then(|v| v.as_str())
                            .map(String::from);
                    }

                    let message = parse_message(&entry, Role::Assistant);
                    current_turn.messages.push(message);
                    last_request_id = request_id;
                }
                "summary" => {
                    // Extract session metadata from summary
                    if session.metadata.session_id.is_none() {
                        session.metadata.session_id = entry
                            .get("sessionId")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                    }
                    // Extract timestamp
                    if session.metadata.timestamp.is_none() {
                        session.metadata.timestamp = entry
                            .get("timestamp")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                    }
                }
                _ => {}
            }
        }

        // Flush final turn
        if !current_turn.messages.is_empty() {
            if let Some(req_id) = &last_request_id
                && let Some(usage) = request_tokens.remove(req_id)
            {
                current_turn.token_usage = Some(usage);
            }
            session.turns.push(current_turn);
        }

        // Set provider
        session.metadata.provider = Some("anthropic".to_string());

        Ok(session)
    }
}

/// Parse a JSONL entry into a Message.
fn parse_message(entry: &Value, role: Role) -> Message {
    let mut content_blocks = Vec::new();

    // Content can be a bare string (human-typed prompts) or an array of content blocks
    // (tool results, assistant text blocks, etc.)
    let content_value = entry.get("message").and_then(|m| m.get("content"));

    if let Some(text) = content_value.and_then(|c| c.as_str()) {
        if !text.is_empty() {
            content_blocks.push(ContentBlock::Text {
                text: text.to_string(),
            });
        }
    } else if let Some(content) = content_value.and_then(|c| c.as_array()) {
        for block in content {
            let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match block_type {
                "text" => {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        content_blocks.push(ContentBlock::Text {
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
                    content_blocks.push(ContentBlock::ToolUse { id, name, input });
                }
                "tool_result" => {
                    let tool_use_id = block
                        .get("tool_use_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let result_content = block
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let is_error = block
                        .get("is_error")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    content_blocks.push(ContentBlock::ToolResult {
                        tool_use_id,
                        content: result_content,
                        is_error,
                    });
                }
                "thinking" => {
                    if let Some(text) = block.get("thinking").and_then(|v| v.as_str()) {
                        content_blocks.push(ContentBlock::Thinking {
                            text: text.to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    Message {
        role,
        content: content_blocks,
        timestamp: entry
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from),
    }
}
