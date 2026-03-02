# CLI Commands

## Command Structure

Normalize has 18 top-level commands organized by domain:

### Core Operations
| Command | Description |
|---------|-------------|
| `view` | View directory/file/symbol structure |
| `grep` | Fast ripgrep-based text search |
| `edit` | Structural code modifications (delete, replace, swap, insert, undo, redo, history) |
| `analyze` | Codebase analysis (40 subcommands) |
| `syntax` | AST inspection and syntax rules (ast, query, rules) |

### Infrastructure
| Command | Description |
|---------|-------------|
| `facts` | Extract and query code facts (symbols, imports, calls) |
| `daemon` | Background process management |
| `grammars` | Tree-sitter grammar management |
| `init` | Initialize normalize in a directory |
| `update` | Check for and install updates |

### Ecosystem Integration
| Command | Description |
|---------|-------------|
| `sessions` | Agent session logs (Claude Code, Codex, Gemini, Normalize) |
| `package` | Package management (info, list, tree, outdated) |
| `tools` | External tool orchestration (lint, test) |
| `serve` | Server protocols (mcp, http, lsp) |
| `generate` | Code generation from API spec |
| `translate` | Translate code between programming languages |

### Utility
| Command | Description |
|---------|-------------|
| `aliases` | List filter aliases |
| `context` | Show directory context (.context.md files) |

## Design Principles

### One namespace per concept
- `aliases` not `filter aliases` (filter does nothing else)

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

[facts]
# See docs/cli/facts.md for rule overrides

[aliases]
tests = ["*_test.go", "**/__tests__/**"]

[serve]
http_port = 8080
```

## See Also

- Individual command docs in this directory
- `docs/cli-design.md` for design rationale
