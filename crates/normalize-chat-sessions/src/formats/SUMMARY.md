# normalize-chat-sessions/src/formats

Session source plugin implementations.

## Architecture

Each format implements the `SessionSource` trait (Phase 1 redesign, replacing the former `LogFormat` trait):
- `sessions_root(project)` — resolves the directory where sessions live
- `discover(root)` — enumerates `SessionRef` entries without full parsing
- `load(r)` — fully parses a `SessionRef` into a `Session`

## Files

- `mod.rs` — `SessionSource` trait, `SessionRef`, `SessionLocation`, `DiscoverError`, `ParseError`, `FormatRegistry`, global registry functions (`register`, `get_format`, `detect_format`, `list_formats`), free-function shims (`parse_session`, `parse_session_with_format`, `project_metadata_roots`), helper functions (`list_jsonl_sessions`, `list_subagent_sessions`)
- `anthropic_history.rs` — shared parser for `api_conversation_history.json` (Anthropic `MessageParam[]` format); exports `load_from_task_dir` and `discover_task_dirs` used by cline + roo-code (gated: `any(format-cline, format-roo)`)
- `claude_code.rs` — Claude Code JSONL format (gated: `format-claude`)
- `cline.rs` — Cline (`saoudrizwan.claude-dev`) directory-per-task format; uses `anthropic_history` (gated: `format-cline`)
- `normalize_agent.rs` — Normalize @agent JSONL format (gated: `format-normalize`)
- `roo_code.rs` — Roo-Code (`rooveterinaryinc.roo-cline`) directory-per-task format with extended `ApiMessage` fields (`ts`, `isSummary`, `reasoning_content`); uses `anthropic_history` (gated: `format-roo`)
- `codex.rs` — OpenAI Codex CLI rollout JSONL format; parses `sessions/YYYY/MM/DD/rollout-*.jsonl`, maps `response_item` lines (message/reasoning/function_call/function_call_output), `parent_thread_id` → subagent linking (gated: `format-codex`)
- `gemini_cli.rs` — Gemini CLI JSONL format; parses `~/.gemini/tmp/<hash>/chats/session-*.jsonl` (main) and `chats/<parent-id>/*.jsonl` (subagents); maps MessageRecord user/gemini types with thoughts→Thinking, toolCalls→ToolUse+ToolResult (gated: `format-gemini`)
- `opencode.rs` — OpenCode SQLite source; reads `$XDG_DATA_HOME/opencode/opencode.db` (or `OPENCODE_DB`) via libsql; `detect` checks SQLite magic + `session`/`session_message` tables; `discover` returns one `SessionRef` per session row using `SessionLocation::Database`; `load` reconstructs Session/Turn/Message/ContentBlock from `session_message` JSON data; async bridged to sync via `block_on` (mirrors ca_cache.rs pattern); NOT in `formats-all` (gated: `format-opencode`)
