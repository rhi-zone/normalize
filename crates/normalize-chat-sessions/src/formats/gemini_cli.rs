//! Gemini CLI JSONL session format parser.
//!
//! Gemini CLI records sessions as JSONL files under:
//!   `~/.gemini/tmp/<project-hash>/chats/session-{date}-{id[:8]}.jsonl`   (main)
//!   `~/.gemini/tmp/<project-hash>/chats/<parent-id>/<session-id>.jsonl`   (subagent)
//!
//! Each file is a JSONL stream:
//! - Line 1: partial metadata `{sessionId, projectHash, startTime, ...}`
//! - Subsequent lines: discriminated records:
//!   - MessageRecord:        `{id, timestamp, content, type: "user"|"gemini"|..., ...}`
//!   - MetadataUpdateRecord: `{$set: {...}}`
//!   - RewindRecord:         `{$rewindTo: "..."}`
//!
//! `gemini` messages may include `toolCalls`, `thoughts`, and `tokens`.
//!
//! Reference: `gemini-cli/packages/core/src/services/chatRecordingTypes.ts`
//!            `gemini-cli/packages/core/src/config/storage.ts`

use super::{DiscoverError, ParseError, SessionLocation, SessionRef, SessionSource, peek_lines};
use crate::{ContentBlock, Message, Role, Session, TokenUsage, Turn};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Gemini CLI session format (JSONL per-session files).
pub struct GeminiCliFormat;

impl SessionSource for GeminiCliFormat {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn sessions_root(&self, _project: Option<&Path>) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".gemini").join("tmp")
    }

    /// Returns 1.0 if the first line has `{sessionId, projectHash}` fields.
    fn detect(&self, path: &Path) -> f64 {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "jsonl" {
            return 0.0;
        }
        for line in peek_lines(path, 1) {
            if let Ok(entry) = serde_json::from_str::<Value>(&line)
                && entry.get("sessionId").is_some()
                && entry.get("projectHash").is_some()
            {
                return 1.0;
            }
        }
        0.0
    }

    /// Walk `root/<project-hash>/chats/` for main sessions and
    /// `root/<project-hash>/chats/<parent-id>/` for subagent sessions.
    fn discover(&self, root: &Path) -> Result<Vec<SessionRef>, DiscoverError> {
        let mut refs = Vec::new();
        let Ok(project_hashes) = std::fs::read_dir(root) else {
            return Ok(refs);
        };

        for hash_entry in project_hashes.filter_map(|e| e.ok()) {
            let hash_dir = hash_entry.path();
            if !hash_dir.is_dir() {
                continue;
            }
            let chats_dir = hash_dir.join("chats");
            if !chats_dir.is_dir() {
                continue;
            }
            let Ok(chats_entries) = std::fs::read_dir(&chats_dir) else {
                continue;
            };
            for chat_entry in chats_entries.filter_map(|e| e.ok()) {
                let chat_path = chat_entry.path();

                if chat_path.is_dir() {
                    // Subagent directory: chats/<parent-id>/<session-id>.jsonl
                    let parent_id = chat_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(String::from);
                    let Ok(sub_entries) = std::fs::read_dir(&chat_path) else {
                        continue;
                    };
                    for sub_entry in sub_entries.filter_map(|e| e.ok()) {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                            continue;
                        }
                        let Ok(meta) = sub_path.metadata() else {
                            continue;
                        };
                        let Ok(mtime) = meta.modified() else {
                            continue;
                        };
                        let session_id = read_session_id_from_jsonl(&sub_path);
                        refs.push(SessionRef {
                            format: self.name(),
                            location: SessionLocation::File(sub_path.clone()),
                            path: sub_path,
                            mtime,
                            parent_session_id: parent_id.clone(),
                            agent_id: session_id,
                            subagent_type: Some("subagent".into()),
                        });
                    }
                } else if chat_path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    // Main session file: chats/session-*.jsonl
                    let Ok(meta) = chat_path.metadata() else {
                        continue;
                    };
                    let Ok(mtime) = meta.modified() else {
                        continue;
                    };
                    let session_id = read_session_id_from_jsonl(&chat_path);
                    refs.push(SessionRef {
                        format: self.name(),
                        location: SessionLocation::File(chat_path.clone()),
                        path: chat_path,
                        mtime,
                        parent_session_id: None,
                        agent_id: session_id,
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
        parse_gemini_jsonl(path)
    }
}

/// Read the first line of a Gemini JSONL file and return the `sessionId`.
fn read_session_id_from_jsonl(path: &Path) -> Option<String> {
    for line in peek_lines(path, 1) {
        if let Ok(entry) = serde_json::from_str::<Value>(&line) {
            return entry
                .get("sessionId")
                .and_then(|v| v.as_str())
                .map(String::from);
        }
    }
    None
}

/// Parse a Gemini JSONL session file into a `Session`.
///
/// Record types (discriminated by field presence):
/// - `{$rewindTo}` → RewindRecord: skip
/// - `{$set}` → MetadataUpdateRecord: skip
/// - `{id, type}` → MessageRecord: process
///
/// MessageRecord types:
/// - `"user"` → User message, starts a new Turn
/// - `"gemini"` → Assistant message with optional `toolCalls`, `thoughts`, `tokens`
/// - `"info" | "error" | "warning"` → skip (system messages)
fn parse_gemini_jsonl(path: &Path) -> Result<Session, ParseError> {
    let file = File::open(path).map_err(|e| ParseError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let reader = BufReader::new(file);

    let mut session = Session::new(path.to_path_buf(), "gemini");
    let mut current_turn = Turn::default();
    let mut first_line = true;

    for raw in reader.lines() {
        let raw = raw.map_err(|e| ParseError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        if raw.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };

        if first_line {
            first_line = false;
            // Line 1: PartialMetadataRecord — {sessionId, projectHash, startTime, ...}
            session.metadata.session_id = record
                .get("sessionId")
                .and_then(|v| v.as_str())
                .map(String::from);
            session.metadata.timestamp = record
                .get("startTime")
                .and_then(|v| v.as_str())
                .map(String::from);
            session.metadata.provider = Some("google".to_string());
            // kind field: "main" | "subagent"
            if let Some(kind) = record.get("kind").and_then(|v| v.as_str()) {
                session.subagent_type = Some(kind.to_string());
            }
            continue;
        }

        // RewindRecord: {$rewindTo: "..."}
        if record.get("$rewindTo").is_some() {
            continue;
        }
        // MetadataUpdateRecord: {$set: {...}}
        if let Some(set_val) = record.get("$set") {
            // Pick up model from metadata updates if available.
            if session.metadata.model.is_none()
                && let Some(model) = set_val.get("model").and_then(|v| v.as_str())
            {
                session.metadata.model = Some(model.to_string());
            }
            continue;
        }

        // MessageRecord: has {id, type, content, timestamp}
        if record.get("id").is_none() {
            continue;
        }
        let msg_type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = record
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from);

        match msg_type {
            "user" => {
                if !current_turn.messages.is_empty() {
                    session.turns.push(std::mem::take(&mut current_turn));
                }
                let content = extract_part_list_union(&record);
                current_turn.messages.push(Message {
                    role: Role::User,
                    content,
                    timestamp,
                });
            }

            "gemini" => {
                // Update model if we haven't seen it yet.
                if session.metadata.model.is_none() {
                    session.metadata.model = record
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }

                let mut content: Vec<ContentBlock> = Vec::new();

                // Thoughts → Thinking blocks
                if let Some(thoughts) = record.get("thoughts").and_then(|t| t.as_array()) {
                    for thought in thoughts {
                        // ThoughtSummary & { timestamp } = { subject, description, timestamp }
                        let subject = thought
                            .get("subject")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let description = thought
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let thinking = if subject.is_empty() {
                            description.to_string()
                        } else {
                            format!("**{subject}**\n{description}")
                        };
                        if !thinking.is_empty() {
                            content.push(ContentBlock::Thinking { text: thinking });
                        }
                    }
                }

                // Main text content
                let text_blocks = extract_part_list_union(&record);
                content.extend(text_blocks);

                // Tool calls → ToolUse + ToolResult pairs
                if let Some(tool_calls) = record.get("toolCalls").and_then(|t| t.as_array()) {
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
                        let input = tc
                            .get("args")
                            .cloned()
                            .unwrap_or(Value::Object(Default::default()));

                        content.push(ContentBlock::ToolUse {
                            id: id.clone(),
                            name,
                            input,
                        });

                        // If a result is present, emit a ToolResult immediately after.
                        if let Some(result_val) = tc.get("result") {
                            let result_text = extract_part_list_union_to_string(result_val);
                            let is_error = tc
                                .get("status")
                                .and_then(|s| s.as_str())
                                .map(|s| s == "error")
                                .unwrap_or(false);
                            content.push(ContentBlock::ToolResult {
                                tool_use_id: id,
                                content: result_text,
                                is_error,
                            });
                        }
                    }
                }

                current_turn.messages.push(Message {
                    role: Role::Assistant,
                    content,
                    timestamp,
                });

                // Token usage
                if let Some(tokens) = record.get("tokens") {
                    current_turn.token_usage = Some(TokenUsage {
                        input: tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0),
                        output: tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0),
                        cache_read: tokens.get("cached").and_then(|v| v.as_u64()),
                        cache_create: None,
                        model: session.metadata.model.clone(),
                    });
                }
            }

            // "info" | "error" | "warning" — system-level, not user-facing conversation
            _ => {}
        }
    }

    if !current_turn.messages.is_empty() {
        session.turns.push(current_turn);
    }

    Ok(session)
}

/// Extract text `ContentBlock`s from a record's `content` field.
///
/// Gemini's `content` is a `PartListUnion` = `string | Part | Part[]` where
/// `Part = {text: string} | {inlineData: ...} | ...`.
fn extract_part_list_union(record: &Value) -> Vec<ContentBlock> {
    let Some(content) = record.get("content") else {
        return Vec::new();
    };
    let text = extract_part_list_union_to_string(content);
    if text.is_empty() {
        Vec::new()
    } else {
        vec![ContentBlock::Text { text }]
    }
}

/// Convert a `PartListUnion` value to a plain string.
fn extract_part_list_union_to_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(s) => Some(s.clone()),
                Value::Object(_) => part.get("text").and_then(|t| t.as_str()).map(String::from),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(_) => val
            .get("text")
            .and_then(|t| t.as_str())
            .map(String::from)
            .unwrap_or_default(),
        _ => String::new(),
    }
}
