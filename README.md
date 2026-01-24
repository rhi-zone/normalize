# Normalize

Fast code intelligence CLI. Structural awareness of codebases through AST-based analysis.

## Install

```bash
# From source
cargo install --path crates/normalize

# Or build locally
cargo build --release
./target/release/normalize --help

# With Nix
nix develop
cargo build --release
```

## Quick Start

```bash
# View project structure
normalize view

# View a specific file's symbols
normalize view src/main.rs

# View a specific symbol
normalize view src/main.rs/main

# Analyze codebase health
normalize analyze health

# Search for text patterns
normalize text-search "TODO"

# Run linters
normalize tools lint
```

## Commands

### view - Navigate Code Structure

View directories, files, and symbols as a unified tree:

```bash
normalize view                       # Current directory tree
normalize view src/                  # Specific directory
normalize view src/main.rs           # File with symbols
normalize view src/main.rs/MyClass   # Specific symbol
normalize view src/main.rs -d 2      # Depth 2 (show nested symbols)
normalize view --full src/foo.rs/bar # Full source code of symbol
normalize view --deps src/foo.rs     # Show imports/exports
normalize view --focus src/foo.rs    # Resolve and show imported symbols
```

### analyze - Codebase Analysis

Unified analysis with subcommands:

```bash
normalize analyze health             # Codebase metrics and health score
normalize analyze complexity         # Cyclomatic complexity report
normalize analyze length             # Function length analysis
normalize analyze security           # Security vulnerability scan
normalize analyze hotspots           # Git history analysis (churn + complexity)
normalize analyze duplicate-functions # Detect code clones
normalize analyze duplicate-types    # Detect duplicate type definitions
normalize analyze docs               # Documentation coverage
normalize analyze all                # Run all analysis passes
```

### tools - Linters and Test Runners

Unified interface to linters, formatters, and type checkers:

```bash
normalize tools lint                 # Auto-detect and run relevant tools
normalize tools lint --fix           # Auto-fix where possible
normalize tools lint --sarif         # Output in SARIF format
normalize tools lint --category type # Only type checkers
normalize tools lint --tools ruff,clippy # Specific tools
normalize tools lint --list          # List available tools

normalize tools test                 # Run native test runners
```

Supported tools: ruff, clippy, rustfmt, oxlint, biome, prettier, tsc, mypy, pyright, eslint, gofmt, go-vet, deno-check, and more.

### text-search - Search Code

Fast ripgrep-based search:

```bash
normalize text-search "pattern"            # Search all files
normalize text-search "TODO" --only "*.rs" # Filter by extension
normalize text-search "fn main" -i         # Case insensitive
normalize text-search "error" --limit 50   # Limit results
```

### package - Package Management

Query package registries and analyze dependencies:

```bash
normalize package info tokio         # Package info from registry
normalize package list               # List project dependencies
normalize package tree               # Dependency tree
normalize package outdated           # Check for updates
normalize package why tokio          # Why is this dependency included?
normalize package audit              # Security vulnerability scan
```

Supports: Cargo, npm, pip, Go modules, Bundler, Composer, Hex, Maven, NuGet, Nix, Conan.

### serve - Server Modes

Run normalize as a server for integration:

```bash
normalize serve mcp                  # MCP server for LLM tools (stdio)
normalize serve http --port 8080     # REST API server
normalize serve lsp                  # LSP server for IDEs
```

### index - Manage Index

Control the file and symbol index:

```bash
normalize index status               # Index stats
normalize index refresh              # Refresh file index
normalize index reindex              # Full reindex
normalize index reindex --call-graph # Include call graph
```

### script - Lua Scripts

Run Lua scripts for automation:

```bash
normalize script run my_script.lua   # Run a Lua script
normalize script list                # List available scripts
```

### sessions - Session Analysis

Analyze Claude Code and other agent session logs:

```bash
normalize sessions                   # List recent sessions
normalize sessions <id>              # Show session details
normalize sessions <id> --analyze    # Full session analysis
normalize sessions --serve           # Web viewer at localhost:3939
```

## Configuration

Create `.normalize/config.toml`:

```toml
[index]
enabled = true

[view]
depth = 1
line_numbers = false

[filter.aliases]
tests = ["**/test_*.py", "**/*_test.go"]
```

### Custom Lint Tools

Add custom tools in `.normalize/tools.toml`:

```toml
[[tools]]
name = "my-linter"
command = ["my-linter", "--format", "json"]
category = "linter"
languages = ["python"]
output_format = "sarif"
```

## Output Formats

Most commands support `--json` for structured output:

```bash
normalize view src/main.rs --json
normalize analyze health --json
normalize tools lint --json
```

## Language Support

Normalize supports 98 languages via tree-sitter grammars including:
Python, Rust, TypeScript, JavaScript, Go, Java, C, C++, Ruby, PHP, Swift, Kotlin, Scala, and many more.

## Development

```bash
# Build
cargo build

# Test
cargo test

# Build grammars (required for tests)
cargo xtask build-grammars

# Install locally
cargo install --path crates/normalize
```

### Prerequisites

- Rust toolchain (1.75+)
- `bun` or `npm` - required to build the sessions web viewer SPA

If using Nix: `nix develop` provides all dependencies.

## License

MIT
