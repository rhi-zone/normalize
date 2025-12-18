# Moss for LLMs

Quick reference for AI agents working with codebases using Moss.

## Quick Start

```bash
# Get instant project snapshot (most token-efficient)
moss --compact overview

# Example output:
# health: B (82%) | deps: 5 direct, 2 dev | docs: 45% | todos: 3 pending | refs: ok
```

## Essential Commands

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `moss --compact overview` | Project health snapshot | First thing when entering a codebase |
| `moss skeleton src/` | Code structure (classes, functions) | Understanding architecture |
| `moss deps FILE` | Import/export analysis | Before modifying a file |
| `moss external-deps` | PyPI/npm dependency tree | Checking for vulns, bloat |
| `moss summarize` | Codebase summary | Getting oriented |
| `moss check-refs` | Find broken references | After refactoring |

## Output Modes

```bash
moss CMD                 # Human-readable (tables, colors)
moss --compact CMD       # Token-efficient one-liner
moss --json CMD          # Full structured data
moss --jq '.field' CMD   # Extract specific fields
```

## Presets for CI/Quick Checks

```bash
moss overview --preset ci      # health + deps, strict mode
moss overview --preset quick   # just health check
moss overview --preset full    # all checks
```

## Common Workflows

**Starting work on a codebase:**
```bash
moss --compact overview        # Quick health check
moss skeleton src/             # See structure
moss summarize                 # Get summary
```

**Before modifying a file:**
```bash
moss deps src/module.py        # What does it import/export?
moss skeleton src/module.py    # What's in it?
```

**After making changes:**
```bash
moss check-refs                # Any broken imports?
moss --compact health          # Did we break anything?
```

**Checking dependencies:**
```bash
moss external-deps --check-vulns    # Security vulnerabilities
moss external-deps --warn-weight 5  # Heavy dependencies
```

## Key Insights

- Use `--compact` for token-efficient output (saves context)
- `skeleton` shows structure without reading full files
- `deps` shows what a file needs and provides
- `overview` combines health, deps, docs, todos, refs checks
- `--jq` lets you extract specific fields from JSON output
