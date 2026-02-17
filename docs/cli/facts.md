# normalize facts

Manage code facts — the file index containing symbols, imports, calls, and other relationships extracted from source code.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `rebuild` | Rebuild the file index |
| `stats` | Show index statistics |
| `files` | List indexed files |
| `packages` | Index external packages into global cache |
| `rules` | Run compiled rule packs (dylibs) against facts |
| `check` | Run Datalog rules (.dl) against facts |

## rebuild

Rebuild the file index from scratch. Scans source files and extracts symbols, calls, and imports.

```bash
normalize facts rebuild
normalize facts rebuild --include symbols
normalize facts rebuild --include symbols --include calls
```

Options:
- `--include <WHAT>` — What to extract: `symbols`, `calls`, `imports` (default: all). Can be repeated.

## stats

Show index statistics — database size, file counts, and breakdown of extracted facts.

```bash
normalize facts stats
normalize facts stats --storage
normalize facts stats --json
```

Options:
- `--storage` — Show storage usage for index and caches

## files

List files in the index, with optional prefix filtering.

```bash
normalize facts files
normalize facts files src/
normalize facts files --limit 50
```

Arguments:
- `[PREFIX]` — Filter files by path prefix

Options:
- `-l, --limit <N>` — Maximum number of files to show (default: 100)

## packages

Index external packages (stdlib, site-packages, node_modules, etc.) into a global cache for cross-reference resolution.

```bash
normalize facts packages
normalize facts packages --only python
normalize facts packages --only rust --only go
normalize facts packages --clear
```

Options:
- `--only <ECOSYSTEM>` — Ecosystems to index: `python`, `go`, `js`, `deno`, `java`, `cpp`, `rust`. Defaults to all available.
- `--clear` — Clear existing index before re-indexing

## rules

Run compiled rule packs (Rust dylibs built against `normalize-facts-rules-api`) against the extracted facts.

```bash
normalize facts rules
normalize facts rules --list
normalize facts rules --rule god-file
normalize facts rules --pack ./my_rules.so
```

Options:
- `--rule <RULE>` — Run a specific rule (runs all if not specified)
- `--pack <PATH>` — Path to a specific rule pack dylib
- `--list` — List available rules instead of running them

## check

Run Datalog rules (`.dl` files) against the extracted facts. Auto-discovers rule files from builtins, global rules, and project rules.

```bash
normalize facts check
normalize facts check my-rules.dl
normalize facts check --list
```

Arguments:
- `[RULES_FILE]` — Path to a specific `.dl` rules file (auto-discovers if omitted)

Options:
- `--list` — List available rules instead of running them

## Global Options

All subcommands support:
- `-r, --root <PATH>` — Root directory (defaults to current directory)
- `--json` — Output as JSON
- `--jsonl` — Output as JSON Lines
- `--jq <EXPR>` — Filter JSON output with jq expression
- `--pretty` — Human-friendly output with colors
- `--compact` — Compact output without colors

## Facts-Optional Design

All normalize commands work without a facts index — they fall back to filesystem scanning and on-demand parsing. The index provides:

- Faster symbol search across the codebase
- Call graph queries (who calls what)
- Cross-file relationship analysis (fact rules need this)
- Incremental updates

## Config

In `.normalize/config.toml`:

```toml
[facts]
# Override builtin rule settings
[facts.rules."god-file"]
allow = ["**/generated/**"]

[facts.rules."god-class"]
enabled = true
severity = "error"
```

## See Also

- [Fact Rules Writing Guide](../fact-rules.md) — How to write `.dl` rules
- [rules](rules.md) — Unified `normalize rules` command
- [commands](commands.md) — All CLI commands
