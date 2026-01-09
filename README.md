# Moss

Fast code intelligence CLI. Structural awareness of codebases through AST-based analysis.

## Install

```bash
# From source
cargo install --path crates/moss

# Or build locally
cargo build --release
./target/release/moss --help

# With Nix
nix develop
cargo build --release
```

## Quick Start

```bash
# View project structure
moss view

# View a specific file's symbols
moss view src/main.rs

# View a specific symbol
moss view src/main.rs/main

# Analyze codebase health
moss analyze health

# Search for text patterns
moss text-search "TODO"

# Run linters
moss tools lint
```

## Commands

### view - Navigate Code Structure

View directories, files, and symbols as a unified tree:

```bash
moss view                       # Current directory tree
moss view src/                  # Specific directory
moss view src/main.rs           # File with symbols
moss view src/main.rs/MyClass   # Specific symbol
moss view src/main.rs -d 2      # Depth 2 (show nested symbols)
moss view --full src/foo.rs/bar # Full source code of symbol
moss view --deps src/foo.rs     # Show imports/exports
moss view --focus src/foo.rs    # Resolve and show imported symbols
```

### analyze - Codebase Analysis

Unified analysis with subcommands:

```bash
moss analyze health             # Codebase metrics and health score
moss analyze complexity         # Cyclomatic complexity report
moss analyze length             # Function length analysis
moss analyze security           # Security vulnerability scan
moss analyze hotspots           # Git history analysis (churn + complexity)
moss analyze duplicate-functions # Detect code clones
moss analyze duplicate-types    # Detect duplicate type definitions
moss analyze docs               # Documentation coverage
moss analyze all                # Run all analysis passes
```

### tools - Linters and Test Runners

Unified interface to linters, formatters, and type checkers:

```bash
moss tools lint                 # Auto-detect and run relevant tools
moss tools lint --fix           # Auto-fix where possible
moss tools lint --sarif         # Output in SARIF format
moss tools lint --category type # Only type checkers
moss tools lint --tools ruff,clippy # Specific tools
moss tools lint --list          # List available tools

moss tools test                 # Run native test runners
```

Supported tools: ruff, clippy, rustfmt, oxlint, biome, prettier, tsc, mypy, pyright, eslint, gofmt, go-vet, deno-check, and more.

### text-search - Search Code

Fast ripgrep-based search:

```bash
moss text-search "pattern"            # Search all files
moss text-search "TODO" --only "*.rs" # Filter by extension
moss text-search "fn main" -i         # Case insensitive
moss text-search "error" --limit 50   # Limit results
```

### package - Package Management

Query package registries and analyze dependencies:

```bash
moss package info tokio         # Package info from registry
moss package list               # List project dependencies
moss package tree               # Dependency tree
moss package outdated           # Check for updates
moss package why tokio          # Why is this dependency included?
moss package audit              # Security vulnerability scan
```

Supports: Cargo, npm, pip, Go modules, Bundler, Composer, Hex, Maven, NuGet, Nix, Conan.

### serve - Server Modes

Run moss as a server for integration:

```bash
moss serve mcp                  # MCP server for LLM tools (stdio)
moss serve http --port 8080     # REST API server
moss serve lsp                  # LSP server for IDEs
```

### index - Manage Index

Control the file and symbol index:

```bash
moss index status               # Index stats
moss index refresh              # Refresh file index
moss index reindex              # Full reindex
moss index reindex --call-graph # Include call graph
```

### script - Lua Scripts

Run Lua scripts for automation:

```bash
moss script run my_script.lua   # Run a Lua script
moss script list                # List available scripts
```

### sessions - Session Analysis

Analyze Claude Code and other agent session logs:

```bash
moss sessions                   # List recent sessions
moss sessions <id>              # Show session details
moss sessions <id> --analyze    # Full session analysis
moss sessions --serve           # Web viewer at localhost:3939
```

## Configuration

Create `.moss/config.toml`:

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

Add custom tools in `.moss/tools.toml`:

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
moss view src/main.rs --json
moss analyze health --json
moss tools lint --json
```

## Language Support

Moss supports 98 languages via tree-sitter grammars including:
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
cargo install --path crates/moss
```

### Prerequisites

- Rust toolchain (1.75+)
- `bun` or `npm` - required to build the sessions web viewer SPA

If using Nix: `nix develop` provides all dependencies.

## License

MIT
