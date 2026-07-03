# normalize-sessions

The `normalize sessions` command surface: inspect and analyze AI agent session
logs (Claude Code and other formats). Owns the `#[cli]` `SessionsService`, all
session report structs, and their `OutputFormatter` impls; the main `normalize`
crate mounts `service::SessionsService` in one line and does nothing else.

Follows the "crate owns its subcommand" pattern (see `normalize-budget`,
`normalize-cfg`). Parsing/analysis algorithms live in the two feature crates this
crate depends on — `normalize-chat-sessions` (session parsing, format registry)
and `normalize-session-analysis` (behavioral analysis) — re-exported under
`crate::sessions` so the report modules refer to them uniformly. The
`OutputFormatter` trait comes from `normalize-output`, re-exported under
`crate::output`. Pretty-mode resolution reads only the `[pretty]` config section
directly, so this crate does not depend on the main crate.

- `Cargo.toml` — crate manifest; `sessions-web` feature gates the optional
  `axum`/`tokio` web-UI server (`serve.rs`).
- `src/` — implementation (see `src/SUMMARY.md`): `lib.rs` (module wiring, shared
  helpers, `sessions`/`output` re-export modules, `resolve_pretty`), `service.rs`
  (the `#[cli] SessionsService`), and per-subcommand report modules.
