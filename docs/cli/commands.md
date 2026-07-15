# CLI Commands

## Command Structure

`normalize --help` groups commands into four sections. This matches the actual help output.

### Core
| Command | Description |
|---------|-------------|
| `view` | View directory/file/symbol structure |
| `grep` | Fast ripgrep-based text search |
| `edit` | Structural code modifications (delete, replace, swap, insert, undo, redo, log) |
| `rules` | Manage and run analysis rules (syntax + fact) |
| `structure` | Manage structural index (symbols, imports, calls) |
| `kg` | Knowledge graph — three primitives: read (selector → units), write (jq transform → mutate/delete), walk (graph traversal) |
| `init` | Initialize normalize in a directory |
| `aliases` | List all registered aliases (built-in and configured) with syntax, value, description, and status |
| `alias save <name>` | Save the last-run (or `--command`-given) command as a named `@alias` |

### Analysis
| Command | Description |
|---------|-------------|
| `analyze` | Codebase analysis: security, docs, skeleton-diff + per-metric residual |
| `overview` | Codebase dashboards: `overview` (health), `overview --full`, `overview summary`, `overview cross-repo-health` (was `analyze health`/`all`/`summary`/`cross-repo-health`) |
| `architecture` | Architectural structure: coupling, cycles, hub modules, layering, depth (`architecture`, `architecture layering`, `architecture depth-map`) |
| `graph` | Dependency-graph analysis: cycles, blast radius, import paths (`graph`, `graph dependents`, `graph import-path`) |
| `similarity` | Duplicate/near-duplicate code detection: clones, duplicate types, AST fragments (`similarity` incl. `--mode clusters`, `similarity duplicate-types`, `similarity fragments`; index-free) |
| `history` | Statistical code-health from git history: `hotspots`, `coupling`, `ownership`, `contributors`, `activity`, `repo-coupling`, `coupling-clusters` (owned by `normalize-git-history`; repo-wide — distinct from `view history`) |
| `rank` | Rank files/functions by metrics |
| `trend` | Track metrics over git history |
| `ci` | Run all quality checks in one pass |
| `budget` | Enforce diff budgets on PRs |
| `ratchet` | Prevent metric regressions |

### Utilities
| Command | Description |
|---------|-------------|
| `filter` | Filter files by glob patterns; inspect `--exclude`/`--only` aliases (`filter aliases`, `filter matches`) |
| `docs` | Fetch upstream symbol documentation (Rust/Go/Python) into LLM context |
| `context` | Show directory context (.context.md files) |
| `translate` | Translate code between programming languages |
| `guide` | Workflow guides with examples |
| `generate` | Code generation from API spec |
| `package` | Package management (info, list, tree, outdated) |
| `sessions` | Agent session logs (Claude Code, Codex, Gemini, Normalize) |

### Infrastructure
| Command | Description |
|---------|-------------|
| `update` | Check for and install updates |
| `daemon` | Background process management |
| `grammars` | Tree-sitter grammar management |
| `syntax` | Tree-sitter AST inspection and query tools |
| `tools` | External tool orchestration (lint, test) |
| `config` | Inspect and validate config files using JSON Schema |
| `serve` | Server protocols (mcp, http, lsp) |

## Design Principles

### One namespace per concept
- `grep` not `grep search` (grep does nothing but search)
- A namespace earns its place once it holds >1 command: `filter` groups `filter aliases` and `filter matches`.

### Group by domain, not by verb
- `sessions`, `grammars list`, `package list`
- Not: `list-sessions`, `list-grammars`, `list-packages`

### Subcommands for related operations
- `analyze` has 40 subcommands because they're all "analysis"
- Better than 40 top-level commands

### `list` as subcommand, not flag
- Consistent: `grammars list`, `daemon list`, `package list`

### Positional args for primary targets
- `normalize view src/main.rs` not `normalize view --file src/main.rs`

## Command Aliases

Familiar names from other tools are rewritten transparently:

| You type | Runs |
|----------|------|
| `normalize search` | `normalize grep` |
| `normalize find` | `normalize grep` |
| `normalize lint` | `normalize rules run` |
| `normalize check` | `normalize ci` |
| `normalize index` | `normalize structure rebuild` |
| `normalize refactor` | `normalize edit` |

## Output Formats

All commands support these global flags:

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--jsonl` | Output as JSON Lines |
| `--jq EXPR` | Filter JSON with jq expression |
| `--pretty` | Human-friendly output with colors |
| `--compact` | LLM-optimized output |

## Configuration

Configuration in `.normalize/config.toml` or `~/.config/normalize/config.toml`:

```toml
[daemon]
enabled = true

[structure]
# See docs/cli/structure.md

[aliases]
tests = ["*_test.go", "**/__tests__/**"]   # override built-in @tests (glob)

[aliases.hotspots]
syntax = "command"
value = "rank complexity --limit 20"
description = "Top 20 most complex functions"

[serve]
http_port = 8080

[walk]
ignore_files = [".gitignore"]       # gitignore-format files to respect
exclude = [".git"]                  # directory names to always skip
```

## See Also

- Individual command docs in this directory
- `docs/cli-design.md` for design rationale
