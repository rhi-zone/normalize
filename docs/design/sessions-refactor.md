# normalize-chat-sessions Refactor: Unified Parsing

**Status: Complete** - Implemented in normalize-chat-sessions, analysis moved to `crates/normalize/src/sessions/analysis.rs`.

## Problem

normalize-chat-sessions previously conflated two concerns:

1. **Parsing** - converting format-specific logs (Claude Code JSONL, Gemini CLI JSON, Codex, Normalize Agent) into structured data
2. **Analysis** - computing statistics (tool call counts, token usage, error patterns, parallelization opportunities)

The `LogFormat` trait's `analyze()` method does both, returning `SessionAnalysis` which contains pre-computed aggregations:

```rust
pub struct SessionAnalysis {
    pub message_counts: HashMap<String, usize>,
    pub tool_stats: HashMap<String, ToolStats>,
    pub token_stats: TokenStats,
    pub error_patterns: Vec<ErrorPattern>,
    pub file_tokens: HashMap<String, u64>,
    pub parallel_opportunities: usize,
    pub total_turns: usize,
}
```

This is problematic because:

- **Analysis is subjective** - what metrics matter depends on the consumer. Iris wants different insights than `normalize sessions`.
- **Iteration requires recompilation** - changing what's analyzed means changing Rust code.
- **Raw data is inaccessible** - consumers can't access the underlying messages/events without re-parsing.

## Design

Split parsing from analysis into two layers:

### 1. Unified Session Type

A format-agnostic representation of session data (from `normalize-chat-sessions/src/session.rs`):

```rust
pub struct Session {
    pub path: PathBuf,
    pub format: String,
    pub metadata: SessionMetadata,
    pub turns: Vec<Turn>,
}

pub struct SessionMetadata {
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub project: Option<String>,
}

pub struct Turn {
    pub messages: Vec<Message>,
    pub token_usage: Option<TokenUsage>,
}

pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub timestamp: Option<String>,
}

pub enum Role {
    User,
    Assistant,
    System,
}

pub enum ContentBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Thinking { text: String },
}

pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: Option<u64>,
    pub cache_create: Option<u64>,
}
```

Helper methods on `Session`:
- `message_count()` - total messages
- `messages_by_role(role)` - count by role
- `tool_uses()` - iterator over (name, input) pairs
- `tool_results()` - iterator over (content, is_error) pairs
- `total_tokens()` - aggregate TokenUsage

### 2. Updated LogFormat Trait

```rust
pub trait LogFormat: Send + Sync {
    fn name(&self) -> &'static str;
    fn sessions_dir(&self, project: Option<&Path>) -> PathBuf;
    fn list_sessions(&self, project: Option<&Path>) -> Vec<SessionFile>;
    fn detect(&self, path: &Path) -> f64;

    // NEW: parse into unified format
    fn parse(&self, path: &Path) -> Result<Session, String>;

    // REMOVED: analyze() - analysis moves to consumers
}
```

### 3. Analysis as Consumer Code

Analysis moves out of normalize-chat-sessions entirely. The `normalize sessions` CLI and other consumers compute their own metrics. Analysis helpers live in normalize-cli or a separate `normalize-chat-sessions-analysis` crate that operates on `Session`.

## Rationale

1. **Separation of concerns** - parsing is objective (bytes → structure), analysis is subjective (structure → insights)

2. **Flexibility** - consumers can compute different metrics without touching the parser.

3. **Performance is fine** - Session files are small (KB-MB). The bottleneck is never "computing stats over hundreds of messages."

4. **Simpler core** - normalize-chat-sessions becomes a pure parser. Smaller API surface, easier to maintain, clearer purpose.

5. **Composability** - Different consumers can share the parser but compute different analyses.

## Migration (Complete)

1. ~~Add `Session` type and `parse()` method to `LogFormat` trait~~ Done
2. ~~Implement `parse()` for each format~~ Done (Claude Code, Gemini CLI, Codex, Normalize Agent)
3. ~~Move analysis logic to normalize CLI~~ Done (`crates/normalize/src/sessions/analysis.rs`)
4. ~~Remove `analyze()` from trait~~ Done
5. ~~`SessionAnalysis` becomes internal to normalize CLI~~ Done

