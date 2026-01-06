# moss text-search

Fast text search using ripgrep.

## Usage

```bash
moss text-search <PATTERN> [OPTIONS]
```

## Examples

```bash
# Search in current directory
moss text-search "fn parse"
moss text-search "TODO|FIXME"

# Case insensitive
moss text-search "config" -i

# With file filtering
moss text-search "impl.*Config" --only "*.rs"
moss text-search "async" --exclude "@tests"

# Limit results
moss text-search "error" --limit 20

# JSON output
moss text-search "Config" --json
moss text-search "Config" --jq '.matches[]'
```

## Options

| Option | Description |
|--------|-------------|
| `-i, --ignore-case` | Case-insensitive search |
| `-l, --limit <N>` | Maximum number of matches to return |
| `--only <PATTERN>` | Include only files matching pattern or @alias |
| `--exclude <PATTERN>` | Exclude files matching pattern or @alias |
| `--json` | Output as JSON |
| `--jq <EXPR>` | Filter JSON with jq expression (implies --json) |
| `--pretty` | Human-friendly output with colors |
| `--compact` | Compact output without colors |
| `-r, --root <PATH>` | Root directory (default: current) |

## Aliases

Moss path aliases work with `--only` and `--exclude`:

```bash
moss text-search "test" --only @tests      # Only test files
moss text-search "config" --exclude @generated
```

## vs ripgrep

`moss text-search` is a thin wrapper around ripgrep with:
- Integration with moss path aliases (`@tests`, `@config`, etc.)
- Consistent output formatting with other moss commands
- JSON output for scripting

For advanced ripgrep features (context lines, file types, word boundaries), use `rg` directly.
