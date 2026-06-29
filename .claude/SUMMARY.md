# .claude/

Claude Code project configuration for this repository.

- `settings.local.json` — local Claude Code settings (MCP servers, hooks, permissions)
- `worktrees/` — temporary agent worktrees created during isolated task execution (excluded from walkers via `[walk] exclude = ["worktrees"]`; not tracked by git). Each is a real git worktree sharing the repo's object database; isolated agents edit only their own worktree copy.
