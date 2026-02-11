# normalize index

Manage the file index and call graph for faster operations.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `status` | Show index status and statistics |
| `rebuild` | Rebuild the index from scratch |
| `refresh` | Update index with changed files |
| `clear` | Remove the index |

## Examples

```bash
# Check index status
normalize index status

# Rebuild everything
normalize index rebuild

# Incremental refresh
normalize index refresh

# Clear index
normalize index clear
```

## Options

**rebuild/refresh:**
- `--call-graph` - Build call graph (default: true)
- `--no-call-graph` - Skip call graph building
- `-r, --root <PATH>` - Root directory

## Index Contents

The index (`.normalize/index.db`) stores:
- File metadata (paths, sizes, modification times)
- Symbols (functions, classes, types)
- Call graph (who calls what)
- Import/export relationships

## Index-Optional Design

All normalize commands work without an index:
- `normalize view` falls back to filesystem + parsing
- `normalize analyze` parses files on demand
- `normalize grep` uses ripgrep directly

The index provides:
- Faster symbol search
- Call graph queries (`analyze callers/callees`)
- Incremental updates

## Config

In `.normalize/config.toml`:

```toml
[index]
# enabled = true      # Enable indexing
# auto_refresh = true # Auto-refresh on changes
```
