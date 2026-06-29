# .claude/

Claude Code project configuration for this repository.

- `settings.json` — project Claude Code settings (MCP servers, hooks, permissions)
- `settings.local.json` — local Claude Code settings (overrides; gitignored)
- `commands/` — project slash commands (available in Claude Code as `/command-name`)
- `scheduled_tasks.lock` — scheduled task registry (managed by Claude Code)
- `worktrees/` — temporary agent worktrees created during isolated task execution (excluded from walkers via `[walk] exclude = [".claude/worktrees/"]`; not tracked by git)
