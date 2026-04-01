# Normalize Quick Reference

Quick reference for working with codebases using Normalize.

## Quick Start

```bash
# Get project overview
normalize analyze health

# View codebase structure
normalize view

# View a specific file's symbols
normalize view src/main.rs
```

## Essential Commands

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `normalize analyze health` | Codebase metrics and health score | First thing when entering a codebase |
| `normalize view src/` | Code structure (symbols, hierarchy) | Understanding architecture |
| `normalize view --deps FILE` | Import/export analysis | Before modifying a file |
| `normalize rank complexity` | Cyclomatic complexity report | Checking code quality |
| `normalize grep "pattern"` | Search code | Finding usage, definitions |
| `normalize package audit` | Security vulnerability scan | Checking dependencies |

## Output Modes

```bash
normalize view FILE           # Human-readable tree format
normalize view FILE --json    # Structured JSON output
```

JSON is useful for parsing but more verbose. Plain text is token-efficient.

## Common Workflows

**Starting work on a codebase:**
```bash
normalize analyze health      # Quick health check
normalize view                # See structure
normalize view src/            # Drill into source
```

**Before modifying a file:**
```bash
normalize view --deps FILE     # What does it import/export?
normalize view FILE            # What symbols are in it?
normalize view FILE/ClassName  # View specific symbol
```

**Understanding a symbol:**
```bash
normalize view FILE/symbol                        # See signature and docstring
normalize view --full FILE/symbol                 # Full source code
normalize analyze call-graph FILE/symbol --callers  # What calls it?
normalize analyze call-graph FILE/symbol --callees  # What does it call?
```

**After making changes:**
```bash
normalize tools lint           # Run linters
normalize analyze health       # Health check
```

**Checking dependencies:**
```bash
normalize package list          # Show dependencies
normalize package tree          # Dependency tree
normalize package audit         # Security vulnerabilities
normalize package why tokio     # Why is this included?
```

**Finding code:**
```bash
normalize grep "TODO"                  # Search for patterns
normalize grep "fn main" -i           # Case insensitive
```

## Key Commands

### view - Navigate Code

```bash
normalize view                     # Project tree
normalize view src/main.rs         # File symbols
normalize view src/main.rs/MyClass # Specific symbol
normalize view --full FILE/symbol  # Full source
normalize view --deps FILE         # Dependencies
normalize view -d 2                # Depth 2 (nested symbols)
```

### analyze - Analysis

```bash
normalize analyze health           # Health metrics
normalize analyze security         # Security scan
normalize analyze call-graph symbol --callers  # What calls this?
normalize analyze call-graph symbol --callees  # What does this call?
normalize analyze graph            # Dependency graph analysis
```

### rank - Ranked Metrics

```bash
normalize rank complexity          # Cyclomatic complexity
normalize rank hotspots            # High-churn files
normalize rank duplicates          # Code clones
normalize rank coupling            # Temporal coupling
normalize rank test-ratio          # Test/impl ratio per module
normalize rank module-health       # Worst modules by combined score
```

### tools - Linters and Test Runners

```bash
normalize tools lint               # Auto-detect and run
normalize tools lint --fix         # Auto-fix issues
normalize tools lint --list        # Available tools
normalize tools test               # Run native test runners
```

### grep - Search

```bash
normalize grep "pattern"            # Full codebase search
normalize grep "TODO" --only "*.rs"
```

### ci - Run All Checks

```bash
normalize ci                       # run all engines, exit 1 on errors
normalize ci --strict              # warnings also fail
normalize ci --no-native           # skip ratchet/budget/stale-summary
normalize ci --sarif               # SARIF output for GitHub Actions
```

## Command Aliases

Familiar names work: `search`/`find` → `grep`, `lint` → `rules run`, `check` → `ci`, `index` → `structure rebuild`, `refactor` → `edit`.

## Key Insights

- `normalize view` is the primary navigation command - works on dirs, files, and symbols
- `normalize analyze` handles health, security, call graphs, docs, and trends; `normalize rank` handles all ranked-list metrics (complexity, hotspots, coupling, duplicates, etc.)
- `normalize grep` for text search, `normalize view` for structural navigation
- Use `--json` when you need to parse output programmatically
- The structure DB (`.normalize/index.sqlite`) caches symbols for fast lookups
- `normalize structure query "<sql>"` runs arbitrary SQL against the index — the fastest way to answer relational questions about the codebase (imports, callers, symbol counts, etc.)
- Three convenience views are available in every index: `entry_points` (public symbols with no callers), `external_deps` (unresolved imports), `external_surface` (public symbols called by files with external deps)
