# src/sessions

Re-export facade for session-related types. Publicly re-exports parsing types from `normalize-chat-sessions` (`Session`, `Turn`, `Message`, `Role`, `ContentBlock`, `LogFormat`, `FormatRegistry`, `TokenUsage`, `SessionFile`, `SessionMetadata`, `parse_session`, `detect_format`, etc.) and all analysis types from `normalize-session-analysis`. Keeps the rest of the crate free from direct multi-crate imports for session data — all session functionality is accessed through this single module.
