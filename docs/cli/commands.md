# CLI Commands

## Command Structure

Normalize has 19 top-level commands organized by domain:

### Core Operations
| Command | Description |
|---------|-------------|
| `view` | View directory/file/symbol structure |
| `edit` | Structural code modifications |
| `history` | Shadow git edit history |
| `analyze` | Codebase analysis (21 subcommands) |
| `text-search` | Fast ripgrep-based text search |

### Infrastructure
| Command | Description |
|---------|-------------|
| `facts` | Manage code facts (file index, symbols, calls, imports) |
| `rules` | Manage and run analysis rules (syntax + fact) |
| `daemon` | Background process management |
| `grammars` | Tree-sitter grammar management |
| `init` | Initialize normalize in a directory |
| `update` | Self-update |

### Ecosystem Integration
| Command | Description |
|---------|-------------|
| `sessions` | Agent session logs (Claude Code, Codex, Gemini, Normalize) |
| `package` | Package management (info, list, tree, why, outdated, audit) |
| `tools` | External tool orchestration (lint, test) |
| `serve` | Server protocols (mcp, http, lsp) |
| `generate` | Code generation (client, types, cli-snapshot) |
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
- `analyze` has 21 subcommands because they're all "analysis"
- Better than 21 top-level commands

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
