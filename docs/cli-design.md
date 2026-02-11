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
Consistent pattern across: `grammars list`, `script list`, `daemon list`, `package list`, `tools lint list`, `tools test list`.
Not: `--list` flag (inconsistent with above).

### 5. Positional args for primary targets
`normalize view src/main.rs` not `normalize view --file src/main.rs`
`normalize sessions <id>` not `normalize sessions --id <id>`

### 6. Flags for modifiers
`--json`, `--pretty`, `--compact` - output format
`--root` - working directory
`--exclude`, `--only` - filtering

### 7. Global flags at root level
Output format flags (`--json`, `--jq`, `--pretty`, `--compact`) are defined once at root, not duplicated per command.

## Entry Points

Total: ~65 entry points (17 top-level + subcommands)

Commands with most subcommands:
- `analyze`: 16 (health, complexity, length, security, docs, files, trace, callers, callees, hotspots, check-refs, stale-docs, check-examples, duplicate-functions, duplicate-types, all)
- `daemon`: 7 (status, stop, start, run, add, remove, list)
- `edit`: 6 (delete, replace, swap, insert, move, copy)
- `package`: 6 (info, list, tree, why, outdated, audit)

Commands with no subcommands (positional/flag-based):
- `view`, `text-search`, `history`, `init`, `update`, `aliases`
