# normalize-chat-sessions

Session log parsing for AI coding agents — converts format-specific log files into a unified `Session` type, separating parsing from analysis.

Supports Claude Code (JSONL), Gemini CLI (JSON), OpenAI Codex CLI (JSONL), and Normalize Agent (JSONL) via the `LogFormat` trait. Key types: `Session`, `Turn`, `Message`, `ContentBlock`, `SessionMetadata`, `Role`. Top-level API: `parse_session(path)` (auto-detect), `parse_session_with_format(path, name)`. Formats register via a global `FORMATS` registry (`register()`, `get_format()`, `detect_format()`); each format is feature-gated. `FormatRegistry` provides an isolated non-global registry for testing.
