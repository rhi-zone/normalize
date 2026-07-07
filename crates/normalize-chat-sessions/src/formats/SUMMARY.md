# normalize-chat-sessions/src/formats

Session source plugin implementations.

## Architecture

Each format implements the `SessionSource` trait (Phase 1 redesign, replacing the former `LogFormat` trait):
- `sessions_root(project)` — resolves the directory where sessions live
- `discover(root)` — enumerates `SessionRef` entries without full parsing
- `load(r)` — fully parses a `SessionRef` into a `Session`

## Files

- `mod.rs` — `SessionSource` trait, `SessionRef`, `SessionLocation`, `DiscoverError`, `ParseError`, `FormatRegistry`, global registry functions (`register`, `get_format`, `detect_format`, `list_formats`), free-function shims (`parse_session`, `parse_session_with_format`, `project_metadata_roots`), helper functions (`list_jsonl_sessions`, `list_subagent_sessions`)
- `claude_code.rs` — Claude Code JSONL format (gated: `format-claude`)
- `normalize_agent.rs` — Normalize @agent JSONL format (gated: `format-normalize`)
- `codex.rs` — OpenAI Codex CLI JSONL format (gated: `format-codex`; TODO phase2: rewrite for current format)
- `gemini_cli.rs` — Gemini CLI JSON format (gated: `format-gemini`; TODO phase2: rewrite for current format)
