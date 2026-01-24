# Moss

**Code intelligence CLI for navigating and analyzing codebases.**

Moss understands code structure (functions, classes, imports) rather than treating code as text. This enables precise navigation, accurate analysis, and structural modifications.

## Quick Start

```bash
# Build from source
git clone https://github.com/rhi-zone/normalize
cd moss
cargo build --release

# Or with Nix
nix develop
cargo build --release

# View a file's structure
moss view src/main.rs

# Analyze codebase health
moss analyze health

# Search for a symbol by name
moss view MyClass
```

## Core Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `view` | Navigate structure | `moss view src/` or `moss view MyClass` |
| `analyze` | Quality metrics | `moss analyze health` or `moss analyze complexity` |
| `tools` | Run linters | `moss tools lint` or `moss tools test` |

## What It Does

**Navigate code structure** - Browse directories, files, and symbols as a unified tree. Find any function or class by name across the entire codebase.

**Analyze code quality** - Health metrics, cyclomatic complexity, function length, duplicate detection, and security scanning.

**Run ecosystem tools** - Unified interface to linters (ruff, clippy, eslint, oxlint), formatters, and type checkers with consistent output.

**98 language support** - Tree-sitter grammars for Python, Rust, TypeScript, JavaScript, Go, Java, C, C++, and many more.

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

## Documentation

- [CLI Reference](/cli/) - All commands and options
- [Language Support](language-support.md) - 98 supported languages
- [Philosophy](philosophy.md) - Design tenets and principles
