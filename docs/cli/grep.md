# normalize grep

Fast text search using ripgrep.

## Usage

```bash
normalize grep <PATTERN> [OPTIONS]
```

## Examples

```bash
# Search in current directory
normalize grep "fn parse"
normalize grep "TODO|FIXME"

# Case insensitive
normalize grep "config" -i

# With file filtering
normalize grep "impl.*Config" --only "*.rs"
normalize grep "async" --exclude "@tests"

# Limit results
normalize grep "error" --limit 20

# JSON output
normalize grep "Config" --json
normalize grep "Config" --jq '.matches[]'
```

## Options

| Option | Description |
|--------|-------------|
| `-i, --ignore-case` | Case-insensitive search |
| `-l, --limit <N>` | Maximum number of matches to return |
| `--only <PATTERN>` | Include only files matching pattern or @alias |
| `--exclude <PATTERN>` | Exclude files matching pattern or @alias |
| `--json` | Output as JSON |
| `--jsonl` | Output one JSON object per line |
| `--jq <EXPR>` | Filter JSON with jq expression (implies --json) |
| `--pretty` | Human-friendly output with colors |
| `--compact` | Compact output without colors |
| `-r, --root <PATH>` | Root directory (default: current) |

## Aliases

Normalize path aliases work with `--only` and `--exclude`:

```bash
normalize grep "test" --only @tests      # Only test files
normalize grep "config" --exclude @generated
```

## vs ripgrep

`normalize grep` is a thin wrapper around ripgrep with:
- Integration with normalize path aliases (`@tests`, `@config`, etc.)
- Consistent output formatting with other normalize commands
- JSON output for scripting

For advanced ripgrep features (context lines, file types, word boundaries), use `rg` directly.
