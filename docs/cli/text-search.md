# normalize text-search

Fast text search using ripgrep.

## Usage

```bash
normalize text-search <PATTERN> [OPTIONS]
```

## Examples

```bash
# Search in current directory
normalize text-search "fn parse"
normalize text-search "TODO|FIXME"

# Case insensitive
normalize text-search "config" -i

# With file filtering
normalize text-search "impl.*Config" --only "*.rs"
normalize text-search "async" --exclude "@tests"

# Limit results
normalize text-search "error" --limit 20

# JSON output
normalize text-search "Config" --json
normalize text-search "Config" --jq '.matches[]'
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

Normalize path aliases work with `--only` and `--exclude`:

```bash
normalize text-search "test" --only @tests      # Only test files
normalize text-search "config" --exclude @generated
```

## vs ripgrep

`normalize text-search` is a thin wrapper around ripgrep with:
- Integration with normalize path aliases (`@tests`, `@config`, etc.)
- Consistent output formatting with other normalize commands
- JSON output for scripting

For advanced ripgrep features (context lines, file types, word boundaries), use `rg` directly.
