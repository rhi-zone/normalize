//! Claude Code JSONL format parser.

use super::{
    LogFormat, ParseError, SessionFile, list_jsonl_sessions, list_subagent_sessions, peek_lines,
};
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
        let claude_dir = if let Ok(dir) = std::env::var("CLAUDE_SESSIONS_DIR") {
            PathBuf::from(dir)
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join(".claude/projects")
        };

        // Claude encodes project paths - check which encoding variant exists
        let path_to_claude_dir = |path: &Path| -> PathBuf {
            let raw = path.to_string_lossy();
            let path_str = raw.trim_end_matches('/').replace('/', "-");
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

    fn list_subagent_sessions(&self, project: Option<&Path>) -> Vec<SessionFile> {
        list_subagent_sessions(&self.sessions_dir(project))
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

    fn parse(&self, path: &Path) -> Result<Session, ParseError> {
        let file = File::open(path).map_err(|e| ParseError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let reader = BufReader::new(file);

        let mut session = Session::new(path.to_path_buf(), self.name());
        let mut current_turn = Turn::default();
        let mut request_tokens: HashMap<String, TokenUsage> = HashMap::new();
        // All requestIds seen in the current turn (one per API call; multi-round turns
        // have multiple calls: tool-call round 1, tool-call round 2, ..., final answer).
        let mut turn_request_ids: Vec<String> = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| ParseError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
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
                    // isMeta: true = caveat/context injections by Claude Code itself (not human input)
                    let is_meta = entry
                        .get("isMeta")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    // Compaction summary injections: bare string starting with the continuation prefix
                    let content_str = entry
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str());
                    let is_compaction_summary = content_str
                        .is_some_and(|s| s.starts_with("This session is being continued"));
                    // Treat these as system-role messages so they don't appear in user output
                    let role = if is_meta || is_compaction_summary {
                        Role::System
                    } else {
                        Role::User
                    };
                    let message = parse_message(&entry, role);
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
                            current_turn.token_usage =
                                sum_turn_tokens(&turn_request_ids, &mut request_tokens);
                            turn_request_ids.clear();
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
                    if let Some(req_id) = request_id
                        && !turn_request_ids.contains(&req_id)
                    {
                        turn_request_ids.push(req_id);
                    }
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
            current_turn.token_usage = sum_turn_tokens(&turn_request_ids, &mut request_tokens);
            session.turns.push(current_turn);
        }

        // Set provider
        session.metadata.provider = Some("anthropic".to_string());

        // Detect subagent metadata from the file path and first entry's fields.
        // Subagent files live at <session-uuid>/subagents/agent-<id>.jsonl
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            && stem.starts_with("agent-")
        {
            session.agent_id = Some(stem.to_string());
            // Parent ID from the grandparent directory name (the session UUID)
            if let Some(parent_dir) = path.parent().and_then(|p| p.parent())
                && let Some(parent_name) = parent_dir.file_name().and_then(|n| n.to_str())
            {
                session.parent_id = Some(parent_name.to_string());
            }
            // Read companion .meta.json for agent type, default to "subagent"
            let meta_path = path.with_extension("meta.json");
            session.subagent_type = Some(
                std::fs::read_to_string(&meta_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .and_then(|v| {
                        v.get("agentType")
                            .and_then(|t| t.as_str())
                            .map(String::from)
                    })
                    .unwrap_or_else(|| "subagent".into()),
            );
        } else {
            session.subagent_type = Some("interactive".into());
        }

        Ok(session)
    }
}

/// Parse a JSONL entry into a Message.
/// Sum token usage across all API calls in a turn.
///
/// A single user-prompt turn may involve multiple API calls (e.g. tool-call
/// rounds before the final answer). Each call has its own `requestId` and its
/// own `usage` entry. We sum them so `Turn::token_usage` reflects the full cost
/// of the turn, not just the last API call.
fn sum_turn_tokens(
    ids: &[String],
    request_tokens: &mut HashMap<String, TokenUsage>,
) -> Option<TokenUsage> {
    if ids.is_empty() {
        return None;
    }
    let mut total = TokenUsage::default();
    let mut any = false;
    for id in ids {
        if let Some(u) = request_tokens.remove(id) {
            total.input += u.input;
            total.output += u.output;
            if let Some(cr) = u.cache_read {
                *total.cache_read.get_or_insert(0) += cr;
            }
            if let Some(cc) = u.cache_create {
                *total.cache_create.get_or_insert(0) += cc;
            }
            // Use the model from the last API call (most likely the final answer)
            if u.model.is_some() {
                total.model = u.model;
            }
            any = true;
        }
    }
    any.then_some(total)
}

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
                    let result_content = match block.get("content") {
                        Some(v) if v.is_string() => v.as_str().unwrap_or("").to_string(),
                        Some(v) => v
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
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
