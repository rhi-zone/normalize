# .claude/

Claude Code project configuration for this repository.

- `settings.json` — committed Claude Code settings (hooks, permissions, MCP servers) shared across the team
- `settings.local.json` — local, per-developer Claude Code settings (overrides; gitignored)
- `commands/` — project-scoped slash-command/skill definitions (available in Claude Code as `/command-name`)
- `scheduled_tasks.lock` — scheduled task registry (managed by Claude Code)
- `worktrees/` — temporary agent worktrees created during isolated task execution (excluded from walkers via `[walk] exclude = [".claude/worktrees/"]`; not tracked by git)
