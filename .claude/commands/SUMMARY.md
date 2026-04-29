# .claude/commands/

Project-scoped Claude Code slash commands. Each `*.md` file is invoked via `/<filename>` (without the `.md`) inside Claude Code sessions and contains the command's prompt template plus any frontmatter (description, allowed tools, etc.).

`handoff.md` — `/handoff` for handing off the current session to a fresh one when the topic is complete or context has grown heavy.

`polish.md` — `/polish` runs a multi-agent codebase polish loop across chosen lenses, persisting state in `POLISH.md`.
