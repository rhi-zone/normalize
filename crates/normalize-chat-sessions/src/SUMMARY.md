# normalize-chat-sessions/src

Source for session log parsing.

- `lib.rs` — crate root; re-exports from `formats` and `session`
- `session.rs` — unified types: `Session`, `Turn`, `Message`, `ContentBlock`, `SessionMetadata`, `Role`, `TokenUsage`
- `formats/` — `LogFormat` trait, global format registry (`register`, `get_format`, `detect_format`), `FormatRegistry`, `parse_session`, `parse_session_with_format`, and per-format implementations
