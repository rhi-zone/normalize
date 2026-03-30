# normalize-chat-sessions/src/formats

Format implementations and the `LogFormat` plugin system.

- `mod.rs` — `LogFormat` trait (`name`, `sessions_dir`, `list_sessions`, `detect`, `parse`); `ParseError` enum (`Io`, `Format`, `Other`) for typed parse errors replacing `String`; global format registry (`FORMATS`, `register`, `get_format`, `detect_format`, `list_formats`); `FormatRegistry` for isolated use; `parse_session`/`parse_session_with_format` returning `Result<Session, ParseError>`; helpers `list_jsonl_sessions`, `peek_lines`, `read_file`
- `claude_code.rs` — `ClaudeCodeFormat`: parses `~/.claude/projects/*/` JSONL logs; skips `isMeta: true` entries (caveat injections) and compaction summary messages (`"This session is being continued..."`) during parse; uses `gix` (pure-Rust) to discover git root for project directory resolution (no `git` binary required)
- `codex.rs` — `CodexFormat`: parses OpenAI Codex CLI JSONL logs
- `gemini_cli.rs` — `GeminiCliFormat`: parses Gemini CLI JSON logs
- `normalize_agent.rs` — `NormalizeAgentFormat`: parses normalize agent JSONL logs
