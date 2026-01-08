# Session Modes & Working Conventions

## Session Modes

### Fresh Mode (default)
Standard collaborative mode. Consider wrapping up when:
- Major feature complete
- 50+ tool calls
- Re-reading files (sign of context degradation)
- Conversation drifted across unrelated topics

Best for: exploratory work, design discussions, uncertain scope.

### Marathon Mode
Continuous autonomous work through TODO.md until empty or blocked.
- Commit after each logical unit (creates resume points)
- Bail out if stuck in a loop (3+ retries on same error)
- Re-reading files repeatedly = context degrading, wrap up soon
- If genuinely blocked, document state in TODO.md and stop

Best for: overnight runs, batch processing TODO items, well-defined tasks.

## Working Style

Start by checking TODO.md. Default: work through ALL items in "Next Up" unless user specifies otherwise.

Agentic by default - continue through tasks unless:
- Genuinely blocked and need clarification
- Decision has significant irreversible consequences
- User explicitly asked to be consulted

When you say "do X first" or "then we can Y" - add it to TODO.md immediately.

Write while researching, not after. Queue review items in TODO.md, don't block for them.

Session handoffs: Add "Next Up" section to TODO.md with 3-5 tasks.

## Context Reset (before `/exit`)
1. Commit current work
2. Move completed tasks to CHANGELOG.md
3. Update TODO.md "Next Up" section
4. Note any open questions

## Updating Files

### CLAUDE.md
Add: workflow patterns, conventions, project-specific knowledge.
Don't add: temporary notes (TODO.md), implementation details (docs/).
Keep it slim: refactor to docs/ when it grows.

### TODO.md
- Next Up: 3-5 concrete tasks for immediate work
- Mark completed as `[x]`, don't delete
- When cleaning up: ONLY delete `[x]` items
- Move completed batches to CHANGELOG.md

### docs/cli/*.md
Update when changing CLI behavior (new flags, changed semantics).

### Commits
Each commit = one logical change. Batch related changes.
