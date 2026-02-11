//! Unified session types for format-agnostic session representation.
//!
//! These types represent parsed session data in a normalized format,
//! allowing consumers to work with sessions regardless of their source
//! format (Claude Code, Gemini CLI, Codex, etc.).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A parsed session in unified format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Session {
    /// Path to the original session file.
    pub path: PathBuf,
    /// Name of the format that parsed this session.
    pub format: String,
    /// Session metadata (IDs, timestamps, provider info).
    pub metadata: SessionMetadata,
    /// Conversation turns (request/response pairs).
    pub turns: Vec<Turn>,
}

/// Session metadata extracted from the log.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SessionMetadata {
    /// Session identifier (format-specific).
    pub session_id: Option<String>,
    /// Session start timestamp.
    pub timestamp: Option<String>,
    /// LLM provider (e.g., "anthropic", "google", "openai").
    pub provider: Option<String>,
    /// Model identifier.
    pub model: Option<String>,
    /// Project path or context.
    pub project: Option<String>,
}

/// A single turn in the conversation (typically one user message + assistant response).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Turn {
    /// Messages in this turn.
    pub messages: Vec<Message>,
    /// Token usage for this turn (if available).
    pub token_usage: Option<TokenUsage>,
}

/// A message from a participant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Message {
    /// Who sent this message.
    pub role: Role,
    /// Message content blocks.
    pub content: Vec<ContentBlock>,
    /// Timestamp of this message (if available).
    pub timestamp: Option<String>,
}

/// Message sender role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

/// A content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content.
    Text { text: String },
    /// Tool invocation by the assistant.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Result of a tool invocation.
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    /// Thinking/reasoning content (e.g., Claude's extended thinking).
    Thinking { text: String },
}

/// Token usage for an API call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TokenUsage {
    /// Input tokens (prompt).
    pub input: u64,
    /// Output tokens (completion).
    pub output: u64,
    /// Tokens read from cache.
    pub cache_read: Option<u64>,
    /// Tokens written to cache.
    pub cache_create: Option<u64>,
    /// Model used for this API call (if known per-turn).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Session {
    /// Create a new empty session.
    pub fn new(path: PathBuf, format: impl Into<String>) -> Self {
        Self {
            path,
            format: format.into(),
            metadata: SessionMetadata::default(),
            turns: Vec::new(),
        }
    }

    /// Total number of messages across all turns.
    pub fn message_count(&self) -> usize {
        self.turns.iter().map(|t| t.messages.len()).sum()
    }

    /// Count messages by role.
    pub fn messages_by_role(&self, role: Role) -> usize {
        self.turns
            .iter()
            .flat_map(|t| &t.messages)
            .filter(|m| m.role == role)
            .count()
    }

    /// Iterate over all tool use blocks.
    pub fn tool_uses(&self) -> impl Iterator<Item = (&str, &serde_json::Value)> {
        self.turns.iter().flat_map(|t| &t.messages).flat_map(|m| {
            m.content.iter().filter_map(|block| match block {
                ContentBlock::ToolUse { name, input, .. } => Some((name.as_str(), input)),
                _ => None,
            })
        })
    }

    /// Iterate over all tool results.
    pub fn tool_results(&self) -> impl Iterator<Item = (&str, bool)> {
        self.turns.iter().flat_map(|t| &t.messages).flat_map(|m| {
            m.content.iter().filter_map(|block| match block {
                ContentBlock::ToolResult {
                    content, is_error, ..
                } => Some((content.as_str(), *is_error)),
                _ => None,
            })
        })
    }

    /// Total token usage across all turns.
    pub fn total_tokens(&self) -> TokenUsage {
        let mut total = TokenUsage::default();
        for turn in &self.turns {
            if let Some(usage) = &turn.token_usage {
                total.input += usage.input;
                total.output += usage.output;
                if let Some(cache_read) = usage.cache_read {
                    *total.cache_read.get_or_insert(0) += cache_read;
                }
                if let Some(cache_create) = usage.cache_create {
                    *total.cache_create.get_or_insert(0) += cache_create;
                }
            }
        }
        total
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
        }
    }
}
