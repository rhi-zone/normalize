# .claude/

Claude Code project configuration for this repository.

- `settings.json` — shared Claude Code settings (hooks, permissions, allowed tools)
- `settings.local.json` — local overrides (MCP servers, local-only permissions)
- `commands/` — custom slash commands for the project
- `worktrees/` — temporary agent worktrees created during isolated task execution (excluded from walkers via `[walk] exclude = ["worktrees"]`; not tracked by git)
