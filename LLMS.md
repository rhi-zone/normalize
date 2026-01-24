# Normalize for LLMs

Quick reference for AI agents working with codebases using Normalize.

## Quick Start

```bash
# Get project overview
normalize analyze --overview

# View codebase structure
normalize view

# View a specific file's symbols
normalize view src/main.rs
```

## Essential Commands

| Command | Purpose | When to Use |
|---------|---------|-------------|
| `normalize analyze --overview` | Project health snapshot | First thing when entering a codebase |
| `normalize view src/` | Code structure (symbols, hierarchy) | Understanding architecture |
| `normalize view --deps FILE` | Import/export analysis | Before modifying a file |
| `normalize analyze --health` | Codebase metrics and health score | Checking project state |
| `normalize text-search "pattern"` | Search code | Finding usage, definitions |
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
normalize analyze --overview   # Quick health check
normalize view                 # See structure
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
normalize view FILE/symbol            # See signature and docstring
normalize view --full FILE/symbol     # Full source code
normalize analyze --calls FILE/symbol # What does it call?
normalize analyze --called-by symbol  # What calls it?
```

**After making changes:**
```bash
normalize lint                  # Run linters
normalize analyze --lint        # Full lint analysis
normalize analyze --health      # Health check
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
normalize text-search "TODO"           # Search for patterns
normalize text-search "fn main" -i     # Case insensitive
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
normalize analyze                  # Health + complexity + security
normalize analyze --overview       # Comprehensive overview
normalize analyze --health         # Health metrics
normalize analyze --complexity     # Cyclomatic complexity
normalize analyze --lint           # Run all linters
normalize analyze --hotspots       # High-churn files
```

### lint - Linters

```bash
normalize lint                     # Auto-detect and run
normalize lint --fix               # Auto-fix issues
normalize lint --list              # Available tools
```

### text-search - Search

```bash
normalize text-search "pattern"            # Full codebase search
normalize text-search "TODO" --only "*.rs"
```

## Key Insights

- `normalize view` is the primary navigation command - works on dirs, files, and symbols
- `normalize analyze` is the primary analysis command - health, complexity, security, lint
- `normalize text-search` for text search, `normalize view` for structural navigation
- Use `--json` when you need to parse output programmatically
- The index (`.normalize/index.sqlite`) caches symbols for fast lookups
