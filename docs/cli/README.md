# CLI Architecture

Normalize provides a unified CLI for code intelligence. Three core primitives: **view**, **edit**, **analyze**.

## Command Categories

### Core Primitives
| Command | Purpose |
|---------|---------|
| [view](view.md) | View directories, files, symbols, line ranges |
| [edit](edit.md) | Structural code modification |
| [analyze](analyze.md) | Code quality analysis (health, complexity, security, duplicates, docs) |

### Search
| Command | Purpose |
|---------|---------|
| [text-search](text-search.md) | Fast ripgrep-based text search |

### Facts & Rules
| Command | Purpose |
|---------|---------|
| [facts](facts.md) | Manage code facts (symbols, imports, calls) |
| [rules](rules.md) | Manage and run analysis rules (syntax + fact) |
| [init](init.md) | Initialize normalize in a project |
| [daemon](daemon.md) | Background daemon for faster operations |

### Ecosystem Tools
| Command | Purpose |
|---------|---------|
| [tools](tools.md) | External tools: [lint](lint.md), [test](test.md) |
| [package](package.md) | Package info, dependency trees, outdated checks |

### Code Generation & Translation
| Command | Purpose |
|---------|---------|
| [generate](generate.md) | Generate code from API specs |
| translate | Translate code between programming languages |

### Utilities
| Command | Purpose |
|---------|---------|
| [grammars](grammars.md) | Manage tree-sitter grammars |
| [sessions](sessions.md) | Analyze agent session logs |
| [update](update.md) | Self-update normalize |
| [aliases](aliases.md) | Manage filter aliases |
| [serve](serve.md) | Start MCP/HTTP/LSP server |
| context | Show directory context (.context.md files) |
| history | Shadow git edit history |

## Global Options

All commands support:
- `--json` - Output as JSON
- `--jsonl` - Output as JSON Lines
- `--jq <EXPR>` - Filter JSON with jq expression (implies --json)
- `--pretty` - Human-friendly output with colors
- `--compact` - Compact output without colors

## Design Principles

1. **Index-optional**: All commands work without an index (graceful degradation via filesystem)
2. **Unified interface**: `normalize view` handles dirs, files, symbols, line ranges
3. **Composable output**: JSON output + jq for scripting
4. **Replace builtin tools**: normalize view/text-search replaces Read/Grep for code-aware operations
