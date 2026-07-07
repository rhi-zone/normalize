# normalize-chat-sessions/src

Session log parsing library for AI coding agents.

## Files

- `lib.rs` — crate root; re-exports `formats::*` and `session::*`
- `session.rs` — core data types: `Session`, `Turn`, `Message`, `Role`, `ContentBlock`, `TokenUsage`, `SessionMetadata`
- `formats/` — session source plugins implementing the `SessionSource` trait
