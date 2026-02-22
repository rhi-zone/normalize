# CLI Design

## Command Structure (17 top-level commands)

### Core Operations
- `view` - View directory/file/symbol structure
- `edit` - Structural code modifications (delete, replace, swap, insert, move, copy)
- `analyze` - Codebase analysis (16 subcommands for different analysis types)
- `text-search` - Fast ripgrep-based text search

### Infrastructure
- `index` - Manage file index (rebuild, stats, files, packages)
- `daemon` - Background process management
- `grammars` - Tree-sitter grammar management
- `init` - Initialize normalize in a directory
- `update` - Self-update

### Ecosystem Integration
- `sessions` - Agent session logs (Claude Code, Codex, Gemini, Normalize)
  - `plans` - Agent-generated plans
- `package` - Package management (info, list, tree, why, outdated, audit)
- `tools` - External tool orchestration
  - `lint` - Linters, formatters, type checkers
  - `test` - Test runners
- `serve` - Server protocols (mcp, http, lsp)
- `generate` - Code generation (client, types)

### Utility
- `aliases` - List filter aliases (used by --exclude/--only)
- `history` - Shadow git edit history
- `script` - Lua script management

## Design Principles

### 1. One namespace per concept
Bad: `filter aliases` (filter does nothing else)
Good: `aliases` (direct access to the one thing)

### 2. Group by domain, not by verb
Bad: `list-sessions`, `list-grammars`, `list-packages`
Good: `sessions`, `grammars list`, `package list`

### 3. Subcommands for related operations
`analyze` has 16 subcommands because they're all "analysis" - one concept with variants.
This is better than 16 top-level commands that pollute the namespace.

### 4. `list` as subcommand, not flag
Consistent pattern across: `grammars list`, `daemon list`, `package list`, `tools lint list`, `tools test list`.
Not: `--list` flag (inconsistent with above).

### 5. Positional args for primary targets
`normalize view src/main.rs` not `normalize view --file src/main.rs`
`normalize sessions <id>` not `normalize sessions --id <id>`

### 6. Flags for modifiers
`--json`, `--pretty`, `--compact` - output format
`--root` - working directory
`--exclude`, `--only` - filtering

### 7. `--dry-run` on every mutating command

Every command that writes, deletes, or modifies anything must support `--dry-run` to preview what would happen without doing it. No exceptions. This applies to `edit`, `init`, `update`, `rules enable/disable`, and anything that touches files, config, or state. Read-only commands (`view`, `analyze`, `text-search`) don't need it.

### 8. Filters compose

Multiple filters always AND together. There are no filter combinations that are invalid or undefined. A user who specifies `--tag debug-print --language rust --enabled` gets exactly the intersection: enabled debug-print rules for Rust. This applies uniformly across all commands that accept filters (`rules list`, `rules run`, `view`, `edit`, etc.).

Corollary: never add a special-cased filter that only works alone or only works with certain other filters. If a filter can't compose, it's a flag, not a filter.

### 9. Global flags at root level
Output format flags (`--json`, `--jq`, `--pretty`, `--compact`) are defined once at root, not duplicated per command.

## Entry Points

Total: ~65 entry points (17 top-level + subcommands)

Commands with most subcommands:
- `analyze`: 16 (health, complexity, length, security, docs, files, trace, callers, callees, hotspots, check-refs, stale-docs, check-examples, duplicate-functions, duplicate-types, all)
- `daemon`: 7 (status, stop, start, run, add, remove, list)
- `rules`: 7 (list, run, tags, enable, disable, add, update, remove)
- `edit`: 6 (delete, replace, swap, insert, move, copy)
- `package`: 6 (info, list, tree, why, outdated, audit)

### `rules` subcommand surface

```
rules list     [--tag <tag>] [--language <lang>] [--enabled] [--disabled] [--type syntax|fact] [--expand]
rules run      [--tag <tag>] [--language <lang>] [--rule <id>] [--fix] [--dry-run]
rules show     <id>
rules tags     [--show-rules] [--tag <tag>]
rules enable   <tag-or-id>   [--dry-run]
rules disable  <tag-or-id>   [--dry-run]
rules add      <url>
rules update
rules remove   <id>
```

`--expand` on `rules list` shows allow patterns, message, and first line of docs per rule. `rules show <id>` renders the full documentation — rationale, examples, remediation, when to disable — accessible offline.

All filters on `list` and `run` compose (see principle #8). `enable`/`disable` accept either a rule ID or a tag name — when given a tag, they apply to all rules matching that tag.

Commands with no subcommands (positional/flag-based):
- `view`, `text-search`, `history`, `init`, `update`, `aliases`
