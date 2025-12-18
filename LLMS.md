# Moss for LLMs

Quick reference for AI agents working with codebases using Moss.

## Quick Start

```bash
# Get instant project snapshot
moss --compact overview

# Example output:
# health: B (82%) - 45 files, 12K lines (45% docs)
#   - docs: Documentation coverage is only 45%
# deps: 5 direct, 2 dev
# docs: 45% coverage
# todos: 3 pending, 8 done
#   - Add input validation to forms
#   - Refactor auth module
# refs: ok
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
moss CMD                 # Human-readable (full details)
moss --compact CMD       # Concise but informative (recommended for LLMs)
moss --json CMD          # Structured data (verbose, not token-efficient)
```

Prefer `--compact` over `--json` - JSON has lots of quotes and braces that waste tokens.

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

- Use `--compact` for informative yet concise output (best for LLMs)
- `overview` shows health, issues, TODOs, and next actions in one command
- `skeleton` shows structure without reading full files
- `deps` shows what a file imports and exports
- Avoid `--json` unless you need structured data - plain text is more token-efficient
