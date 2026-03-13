# CLI Design

## Command Structure (21 top-level commands)

### Core Operations
- `view` - View directory/file/symbol structure
- `grep` - Fast ripgrep-based text search
- `edit` - Structural code modifications (delete, replace, swap, insert, undo, redo, goto, batch, history)
- `analyze` - Codebase analysis (45 subcommands)
- `syntax` - AST inspection (ast, query)
- `rules` - Manage and run analysis rules (syntax + fact)

### Infrastructure
- `structure` - Manage the structural index (symbols, imports, calls)
- `config` - Inspect and validate config files using JSON Schema
- `daemon` - Background process management
- `grammars` - Tree-sitter grammar management
- `init` - Initialize normalize in a directory
- `update` - Check for and install updates

### Ecosystem Integration
- `sessions` - Agent session logs (Claude Code, Codex, Gemini)
  - `list`, `show`, `stats`, `messages`, `plans`
- `package` - Package management (info, list, tree, why, outdated, audit)
- `tools` - External tool orchestration
  - `lint` - Linters, formatters, type checkers
  - `test` - Test runners
- `serve` - Server protocols (mcp, http, lsp)
- `generate` - Code generation from API spec
- `translate` - Translate code between languages
- `guide` - Workflow guides with examples

### Utility
- `aliases` - List filter aliases (used by --exclude/--only)
- `context` - Show directory context (.context.md files)

## Design Principles

### 1. One namespace per concept
Bad: `filter aliases` (filter does nothing else)
Good: `aliases` (direct access to the one thing)

### 2. Group by domain, not by verb
Bad: `list-sessions`, `list-grammars`, `list-packages`
Good: `sessions`, `grammars list`, `package list`

### 3. Subcommands for related operations
`analyze` has 45 subcommands because they're all "analysis" - one concept with variants.
This is better than 40 top-level commands that pollute the namespace.

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

Every command that writes, deletes, or modifies anything must support `--dry-run` to preview what would happen without doing it. No exceptions. This applies to `edit`, `init`, `update`, `rules enable/disable`, and anything that touches files, config, or state. Read-only commands (`view`, `analyze`, `grep`) don't need it.

### 8. Filters compose

Multiple filters always AND together. There are no filter combinations that are invalid or undefined. A user who specifies `--tag debug-print --language rust --enabled` gets exactly the intersection: enabled debug-print rules for Rust. This applies uniformly across all commands that accept filters (`rules list`, `rules run`, `view`, `edit`, etc.).

Corollary: never add a special-cased filter that only works alone or only works with certain other filters. If a filter can't compose, it's a flag, not a filter.

### 9. Global flags at root level
Output format flags (`--json`, `--jq`, `--pretty`, `--compact`) are defined once at root, not duplicated per command.

## Entry Points

Total: ~110 entry points (21 top-level + subcommands)

Commands with most subcommands:
- `analyze`: 45 (health, all, summary, complexity, complexity-trend, length, length-trend, ceremony, duplicates, duplicate-types, uniqueness, call-complexity, fragments, node-types, size, density, density-trend, module-health, imports, depth-map, surface, layering, docs, files, call-graph, trace, architecture, graph, dependents, coupling, coupling-clusters, hotspots, ownership, contributors, activity, repo-coupling, cross-repo-health, provenance, trend, test-ratio, test-gaps, budget, test-ratio-trend, security, skeleton-diff)
- `rules`: 10 (list, run, enable, disable, show, tags, add, update, remove, validate)
- `edit`: 10 (delete, replace, swap, insert, undo, redo, goto, batch, history)
- `daemon`: 7 (status, stop, start, run, add, remove, list)
- `sessions`: 6 (list, show, stats, messages, plans)
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
rules setup
rules validate
```

`--expand` on `rules list` shows allow patterns, message, and first line of docs per rule. `rules show <id>` renders the full documentation — rationale, examples, remediation, when to disable — accessible offline.

All filters on `list` and `run` compose (see principle #8). `enable`/`disable` accept either a rule ID or a tag name — when given a tag, they apply to all rules matching that tag.

Commands with no subcommands (positional/flag-based):
- `view`, `grep`, `aliases`, `context`, `init`, `update`
