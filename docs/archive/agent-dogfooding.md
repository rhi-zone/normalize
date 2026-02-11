# Agent Dogfooding (Historical)

Archived insights from using the normalize agent on itself. Agent functionality now lives in [spore](https://github.com/user/spore).

## What Works

- **Ephemeral context model**: 1-turn output visibility forces explicit memory curation via `$(keep)`/`$(note)`
- **State machine (explorer/evaluator)**: Prevents pre-answering; evaluator curates, explorer acts
- **Session logging**: JSONL logs in `.normalize/agent/logs/` enable post-hoc analysis
- **`normalize sessions --format normalize`**: List/grep agent sessions for debugging
- **Working memory**: Synthesized notes (`$(note)`) survive longer than raw outputs

## Friction Points Discovered

- **View loops**: Agent can view same files repeatedly without extracting info (7x same file, 15 turns, incomplete)
  - Cause: `view` output doesn't contain needed info directly
  - Pattern: succeeds when tool output = answer, struggles when output requires interpretation
- **Text-search syntax confusion**: Agent used grep syntax (`\|`) with text-search despite tool being renamed
  - Shows agents don't understand tool semantics, just syntax
- **Large file edits**: Edit tool match failures on large deletions
- **Context compaction**: Claude Code's auto-compaction lost in-progress work (normalize's dynamic reshaping avoids this)

## Key Insights

- **Role framing beats instructions**: "You are an EVALUATOR" + banned phrases + examples beats instruction-only
- **Concrete examples prevent defaults**: Example in prompt prevents LLM defaulting to XML function calls
- **Context uniqueness**: Identical context between any two LLM calls risks loops
- **Cross-project parallelization**: Running separate Claude Code sessions per project avoids within-project coordination costs

## Session Analysis Workflow

```bash
normalize sessions --format normalize                    # list recent agent sessions
normalize sessions --format normalize --grep "benchmark" # filter by content
normalize sessions <id> --analyze                   # full analysis
```
