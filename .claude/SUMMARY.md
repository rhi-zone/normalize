# .claude/

Claude Code project configuration for this repository.

- `settings.json` — committed Claude Code settings (hooks, permissions) shared across the team
- `settings.local.json` — local, per-developer Claude Code settings (MCP servers, hooks, permissions)
- `commands/` — project-scoped slash-command/skill definitions
- `worktrees/` — temporary agent worktrees created during isolated task execution (excluded from walkers via `[walk] exclude = ["worktrees"]`; not tracked by git)
