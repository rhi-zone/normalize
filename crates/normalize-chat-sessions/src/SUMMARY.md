# normalize-chat-sessions/src

Source for session log parsing.

- `lib.rs` — crate root; re-exports from `formats` and `session`
- `session.rs` — unified types: `Session` (with optional `parent_id`, `agent_id`, `subagent_type` for subagent sessions), `Turn`, `Message`, `ContentBlock`, `SessionMetadata`, `Role`, `TokenUsage`
- `formats/` — `LogFormat` trait (with `metadata_roots` and `list_subagent_sessions` methods), global format registry (`register`, `get_format`, `detect_format`), `project_metadata_roots` free function for sync discovery, `FormatRegistry`, `parse_session`, `parse_session_with_format`, `list_subagent_sessions` helper, and per-format implementations
