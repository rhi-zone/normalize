# Session Modes

Two working modes. To switch: copy the desired mode's rules into CLAUDE.md "Working Style" section.

## Fresh Mode

Standard collaborative mode. Consider wrapping up when:
- Major feature complete
- 50+ tool calls
- Re-reading files (sign of context degradation)
- Conversation drifted across unrelated topics

Best for: exploratory work, design discussions, uncertain scope.

## Marathon Mode

Continuous autonomous work through TODO.md until empty or blocked.
- Commit after each logical unit (creates resume points)
- Bail out if stuck in a loop (3+ retries on same error)
- Re-reading files repeatedly = context degrading, wrap up soon
- If genuinely blocked, document state in TODO.md and stop

Best for: overnight runs, batch processing TODO items, well-defined tasks.
