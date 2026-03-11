# normalize daemon

Manage the background daemon for faster operations.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `start` | Start the daemon |
| `stop` | Stop the daemon |
| `status` | Check daemon status |
| `restart` | Restart the daemon |
| `add` | Add a project root to watch |
| `remove` | Remove a project root from watching |
| `list` | List all watched roots |

## Examples

```bash
normalize daemon start
normalize daemon status
normalize daemon stop
normalize daemon add ~/projects/app
normalize daemon add --dry-run         # preview without applying
normalize daemon remove ~/projects/app
```

## Purpose

The daemon provides:
- Persistent grammar cache (faster parsing)
- File watching for index updates
- Reduced startup overhead for repeated commands

## Config

In `.normalize/config.toml`:

```toml
[daemon]
enabled = true      # Enable daemon
auto_start = true   # Start automatically when needed
```
