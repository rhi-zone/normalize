# normalize sessions

Analyze Claude Code, Codex, Gemini CLI, and Normalize agent session logs.

## Usage

```bash
normalize sessions <SUBCOMMAND> [OPTIONS]
```

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `list` | List available sessions |
| `show` | Show a specific session (summary or full conversation) |
| `stats` | Show aggregate statistics across sessions |
| `messages` | Extract all messages across sessions into a flat, queryable form |
| `plans` | List and view agent plans |

### list

List available sessions:

```bash
normalize sessions list                          # List sessions for current project
normalize sessions list --all-projects           # All projects
normalize sessions list --format codex           # Codex sessions
normalize sessions list --grep "benchmark"       # Filter by content
normalize sessions list --days 7                 # Last 7 days
normalize sessions list --since 2025-01-01       # Since date
normalize sessions list -n 50                    # Limit results
normalize sessions list --project /path/to/repo  # Specific project
```

Options:
- `--format <FORMAT>` — Force specific format: `claude`, `codex`, `gemini`, `normalize`
- `--grep <PATTERN>` — Filter sessions by content pattern
- `--days <N>` — Filter sessions from the last N days
- `--since <DATE>` — Filter sessions since date (YYYY-MM-DD)
- `--until <DATE>` — Filter sessions until date (YYYY-MM-DD)
- `--project <PATH>` — Filter by specific project path
- `--all-projects` — Show sessions from all projects
- `-n, --limit <N>` — Maximum number of sessions

### show

Show a specific session:

```bash
normalize sessions show abc123                   # Session summary
normalize sessions show abc123 --analyze         # Full analysis
normalize sessions show abc123 --full            # Full conversation log
normalize sessions show abc123 --exact           # Exact/prefix match only
normalize sessions show abc123 --format codex    # Force format
```

Arguments:
- `[SESSION]` — Session ID or path

Options:
- `--analyze` — Run full analysis instead of summary
- `--full` — Show full conversation log
- `--exact` — Require exact/prefix match (disable fuzzy)
- `--format <FORMAT>` — Force specific format: `claude`, `codex`, `gemini`, `normalize`

### stats

Show aggregate statistics across sessions:

```bash
normalize sessions stats                         # Stats for current project
normalize sessions stats --all-projects          # All projects
normalize sessions stats --days 30               # Last 30 days
normalize sessions stats --format codex          # Codex sessions
```

Options: same filtering as `list` (`--format`, `--grep`, `--days`, `--since`, `--until`, `--project`, `--all-projects`, `-n`).

### messages

Extract all messages across sessions into a flat, queryable form:

```bash
normalize sessions messages                              # User messages (default)
normalize sessions messages --role all                   # All messages
normalize sessions messages --role assistant              # Assistant only
normalize sessions messages --grep "TODO"                # Filter by content
normalize sessions messages --no-truncate                # Full message text
normalize sessions messages --max-chars 500              # Custom truncation
normalize sessions messages --jq '.[] | select(.role == "user")'
```

Options:
- `--role <ROLE>` — Filter by role: `user` (default), `assistant`, `all`
- `--grep <PATTERN>` — Filter messages by content pattern
- `--max-chars <N>` — Truncate message text to N chars (default: 200)
- `--no-truncate` — Don't truncate message text
- Plus same filtering options as `list`

### plans

List and view agent plans:

```bash
normalize sessions plans                         # List all plans
normalize sessions plans my-plan                 # View specific plan
normalize sessions plans -n 10                   # Limit results
```

Arguments:
- `[NAME]` — Plan name to view (omit to list all)

Options:
- `-n, --limit <N>` — Maximum number of plans

## Formats

| Format | Directory | File Pattern |
|--------|-----------|--------------|
| `claude` | `~/.claude/projects/<encoded-path>/` | `*.jsonl` |
| `codex` | `~/.codex/sessions/YYYY/MM/DD/` | `*.jsonl` |
| `gemini` | `~/.gemini/tmp/<hash>/` | `logs.json` |
| `normalize` | `.normalize/agent/logs/` | `*.jsonl` |
