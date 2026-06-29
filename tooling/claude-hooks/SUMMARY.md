# tooling/claude-hooks/

Claude Code hook scripts that enforce the orchestrator workflow for this repository (installed into `.claude/settings.json` hook slots; jq-free by design since the harness doesn't always have jq on PATH).

- `block-blocking-bash.sh` — PreToolUse(Bash) hook; denies commands that never return on their own (follow/stream/watch) and would hang the session until timeout. Claude opts into long-running commands via `run_in_background: true`.
- `block-mainsession-exploration.sh` — PreToolUse hook enforcing that the main session is a pure orchestrator: limits Bash to git commit/push/status/log and routes all file edits, searches, and other shell to subagents. Uses bash parameter-expansion brace-depth splitting (no JSON walker) to separate harness-controlled fields from model-controlled content; a top-level `agent_id` marks a subagent and bypasses the block.
- `inject-orchestrator-rules.sh` — UserPromptSubmit hook; injects `orchestrator-rules.md` as additionalContext into the main session only (silent for subagents, detected via top-level `agent_id`).
- `post-history.sh` — post hook with a self-contained top-level `agent_id` detector (inlined from `lib/agent-id.sh`).
- `orchestrator-rules.md` — the rules text injected into the main session (delegate edits/searches/shell to subagents).
- `orchestrator-workflows.md` — lessons for running a Workflow from the main session (resume/args caveats, fan-out gating).
- `lib/` — shared shell/awk helpers: `agent-id.sh` (subagent detection), `extract-command.awk`, `tokenize-bash.awk`.
