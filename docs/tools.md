# Normalize Tools

**CLI**: `normalize <command> [options]` — all commands support `--json`

**MCP Server**: `normalize mcp-server`

## Core Primitives

Three tools for all codebase operations:

### view

Show tree, file skeleton, or symbol source.

```
normalize view [target] [options]
```

- `view` — project tree
- `view src/` — directory contents
- `view file.py` — file skeleton (fuzzy paths OK)
- `view file.py/Class` — symbol source
- `--depth N` — expansion depth
- `--deps` — show dependencies
- `--types-only` — only type definitions

### edit

Structural code modifications via subcommands.

```
normalize edit <target> <subcommand>
```

- `delete` — remove symbol
- `replace "code"` — swap content
- `insert "code" --at before|after|prepend|append` — insert relative to target
- `move <dest>` — move symbol
- `swap <other>` — swap two symbols
- `--undo` — undo last edit

### analyze

Codebase analysis via subcommands.

```
normalize analyze [subcommand] [options]
```

- `health` — file counts, line counts, avg complexity
- `complexity` — cyclomatic complexity per function
- `security` — vulnerability scanning
- `callers <symbol>` — what calls this symbol
- `callees <symbol>` — what this symbol calls
- `hotspots` — frequently changed files

## Search & Sessions

### text-search

Regex search using ripgrep.

```
normalize text-search <pattern> [options]
```

- `-i` — case-insensitive
- `-l, --limit <N>` — max results
- `--only <glob>` — filter by filename/path
- `--exclude <glob>` — exclude paths
- `--json` — structured output

### sessions

Analyze agent session logs (Claude Code, Codex, Gemini, Normalize).

```
normalize sessions [session_id] [options]
```

- `--format <fmt>` — `claude` (default), `codex`, `gemini`, `normalize`
- `--grep <pattern>` — filter sessions by content
- `--analyze` — full session analysis
- `--jq <expr>` — apply jq expression

## DWIM Resolution

Tool names are resolved with fuzzy matching. Fuzzy path resolution also works: `dwim.py` → `src/normalize/dwim.py`
