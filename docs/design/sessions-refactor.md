# moss-sessions Refactor: Unified Parsing

## Problem

moss-sessions currently conflates two concerns:

1. **Parsing** - converting format-specific logs (Claude Code JSONL, Gemini CLI JSON, Codex, Moss Agent) into structured data
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

- **Analysis is subjective** - what metrics matter depends on the consumer. Iris wants different insights than `moss sessions`.
- **Iteration requires recompilation** - changing what's analyzed means changing Rust code.
- **Raw data is inaccessible** - consumers can't access the underlying messages/events without re-parsing.

## Design

Split into two layers:

### 1. Unified Session Type (new)

A format-agnostic representation of session data:

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
    Text(String),
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
}

pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: Option<u64>,
    pub cache_create: Option<u64>,
}
```

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

Analysis moves out of moss-sessions entirely. Consumers (moss CLI, spore-sessions Lua bindings, Iris) compute their own metrics:

```lua
-- In Lua (via spore-sessions)
local session = sessions.parse(path)

-- Compute whatever metrics matter to you
local tool_counts = {}
for _, turn in ipairs(session.turns) do
    for _, msg in ipairs(turn.messages) do
        for _, block in ipairs(msg.content) do
            if block.type == "tool_use" then
                tool_counts[block.name] = (tool_counts[block.name] or 0) + 1
            end
        end
    end
end
```

For `moss sessions` CLI, analysis helpers can live in moss-cli or a separate `moss-sessions-analysis` crate that operates on `Session`.

## Rationale

1. **Separation of concerns** - parsing is objective (bytes → structure), analysis is subjective (structure → insights)

2. **Flexibility** - Iris can iterate on what patterns matter without touching Rust. New metrics = new Lua code, not recompilation.

3. **Performance is fine** - Session files are small (KB-MB). LuaJIT analyzing a parsed session is fast enough. The bottleneck is never "computing stats over hundreds of messages."

4. **Simpler core** - moss-sessions becomes a pure parser. Smaller API surface, easier to maintain, clearer purpose.

5. **Composability** - Different consumers can share the parser but compute different analyses. `moss sessions --compact` and Iris insights don't need to agree on what to track.

## Migration

1. Add `Session` type and `parse()` method to `LogFormat` trait
2. Implement `parse()` for each format (extract from current `analyze()` implementations)
3. Move analysis logic to moss CLI (for `moss sessions` command)
4. Deprecate then remove `analyze()` from trait
5. `SessionAnalysis` becomes internal to moss CLI, not part of moss-sessions public API

## spore-sessions Integration

With this refactor, spore-sessions becomes trivial:

```rust
// spore-sessions/src/lib.rs
pub struct SessionsIntegration;

impl Integration for SessionsIntegration {
    fn register(&self, lua: &Lua) -> Result<()> {
        let sessions = lua.create_table()?;

        sessions.set("parse", lua.create_function(|lua, path: String| {
            let session = moss_sessions::parse_session(Path::new(&path))?;
            // Convert Session to Lua table
            session_to_lua(lua, &session)
        })?)?;

        sessions.set("list", lua.create_function(|lua, project: Option<String>| {
            let files = moss_sessions::list_sessions(project.as_deref().map(Path::new));
            // Convert to Lua array
            files_to_lua(lua, &files)
        })?)?;

        sessions.set("formats", lua.create_function(|_, ()| {
            Ok(moss_sessions::list_formats())
        })?)?;

        lua.globals().set("sessions", sessions)?;
        Ok(())
    }
}
```

Lua gets raw session data. Analysis lives in Lua scripts that Iris controls.
