# normalize structure

Manage the structural index (symbols, imports, calls).

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `rebuild` | Rebuild the structural index |
| `stats` | Show index statistics |
| `files` | List indexed files |
| `packages` | Index external packages into global cache |
| `query` | Run an arbitrary SQL query against the structural index |

## rebuild

Rebuild the file index from scratch. Scans source files and extracts symbols, calls, and imports.

```bash
normalize structure rebuild
normalize structure rebuild --include symbols
normalize structure rebuild --include symbols --include calls
```

Options:
- `--include <WHAT>` — What to extract: `symbols`, `calls`, `imports` (default: all). Can be repeated.

## stats

Show index statistics — database size, file counts, and breakdown of extracted facts.

```bash
normalize structure stats
normalize structure stats --storage
normalize structure stats --json
```

Options:
- `--storage` — Show storage usage for index and caches

## files

List files in the index, with optional prefix filtering.

```bash
normalize structure files
normalize structure files src/
normalize structure files --limit 50
```

Arguments:
- `[PREFIX]` — Filter files by path prefix

Options:
- `-l, --limit <N>` — Maximum number of files to show (default: 100)

## packages

Index external packages (stdlib, site-packages, node_modules, etc.) into a global cache for cross-reference resolution.

```bash
normalize structure packages
normalize structure packages --only python
normalize structure packages --only rust --only go
normalize structure packages --clear
```

Options:
- `--only <ECOSYSTEM>` — Ecosystems to index: `python`, `go`, `js`, `deno`, `java`, `cpp`, `rust`. Defaults to all available.
- `--clear` — Clear existing index before re-indexing

## query

Run an arbitrary read-only SQL query against the structural index (`.normalize/index.sqlite`).
Results are returned as a table (text) or JSON array of objects (with `--json`).

The index exposes these tables: `files`, `symbols`, `symbol_attributes`, `symbol_implements`,
`calls`, `imports`, `type_methods`, `type_refs`. Three convenience views are also defined at
index open time:

| View | Description |
|------|-------------|
| `entry_points` | Public symbols with no callers |
| `external_deps` | Imports where `resolved_file IS NULL` |
| `external_surface` | Public symbols called from files that have unresolved imports |

```bash
normalize structure query "SELECT name, kind, file FROM symbols WHERE kind = 'function' LIMIT 10"
normalize structure query "SELECT * FROM entry_points" --json
normalize structure query "SELECT file, COUNT(*) AS n FROM imports GROUP BY file ORDER BY n DESC LIMIT 5"
normalize structure query "SELECT DISTINCT module FROM external_deps ORDER BY module" --jsonl
```

Arguments:
- `<SQL>` — SQL query to run against the structural index

## Global Options

All subcommands support:
- `-r, --root <PATH>` — Root directory (defaults to current directory)
- `--json` — Output as JSON
- `--jsonl` — Output as JSON Lines
- `--jq <EXPR>` — Filter JSON output with jq expression
- `--pretty` — Human-friendly output with colors
- `--compact` — Compact output without colors

## Structure-Optional Design

All normalize commands work without a structural index — they fall back to filesystem scanning and on-demand parsing. The index provides:

- Faster symbol search across the codebase
- Call graph queries (who calls what)
- Cross-file relationship analysis
- Incremental updates

## See Also

- [rules](rules.md) — Rule execution commands
- [commands](commands.md) — All CLI commands
