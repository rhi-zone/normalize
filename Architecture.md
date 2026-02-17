# Normalize Architecture

High-level architecture for in-context learning. See `docs/` for detailed documentation.

## Core Concept

Normalize is a **fast code intelligence CLI** implemented entirely in Rust. It understands code structure (AST, symbols, imports, dependencies) via tree-sitter grammars rather than treating code as raw text.

## Key Components

```
┌─────────────────────────────────────────────────────────────┐
│                     CLI (normalize)                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │     View     │  │     Edit     │  │   Analyze    │      │
│  │  (symbols,   │  │ (structural  │  │ (health,     │      │
│  │   tree)      │  │  refactors)  │  │  complexity) │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         │                 │                 │               │
│         └─────────────────┼─────────────────┘               │
│                           ▼                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Languages   │  │    Facts     │  │    Rules     │      │
│  │  (98 langs,  │  │ (SQLite DB,  │  │ (syntax +    │      │
│  │  tree-sitter)│  │  extraction) │  │  Datalog)    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         │                 │                 │               │
│         └─────────────────┼─────────────────┘               │
│                           ▼                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Shadow Git  │  │  Ecosystems  │  │    Tools     │      │
│  │  (edit       │  │  (deps,      │  │  (linters,   │      │
│  │   history)   │  │   packages)  │  │   runners)   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Crate Organization

30 crates, organized by domain:

```
crates/
├── normalize/                     # Main CLI binary
├── normalize-core/                # Core types and utilities
├── normalize-derive/              # Proc macros
├── normalize-output/              # OutputFormatter trait, format flags
│
├── normalize-languages/           # Language trait (98 implementations)
├── normalize-language-meta/       # Language metadata (extensions, names)
├── normalize-grammars/            # Tree-sitter grammar loading (publish=false)
│
├── normalize-view/                # View command logic
├── normalize-edit/                # Edit command logic
├── normalize-shadow/              # Shadow git (edit history)
├── normalize-filter/              # --exclude/--only filtering
├── normalize-path-resolve/        # Path resolution utilities
│
├── normalize-facts/               # Fact extraction + SQLite storage
├── normalize-facts-core/          # Fact data types (Symbol, Import, etc.)
├── normalize-facts-rules-api/     # Stable ABI for rule plugins (abi_stable)
├── normalize-facts-rules-builtins/# Built-in fact rules (cdylib)
├── normalize-facts-rules-interpret/# Interpreted Datalog rules
│
├── normalize-syntax-rules/        # Tree-sitter query rules (.scm)
├── normalize-rules-loader/        # Rule loading infrastructure
│
├── normalize-deps/                # Dependency analysis
├── normalize-local-deps/          # LocalDeps trait (import resolution)
├── normalize-ecosystems/          # Ecosystem trait (cargo, npm, pip)
├── normalize-package-index/       # PackageIndex trait (apt, brew)
│
├── normalize-tools/               # External tool orchestration
├── normalize-cli-parser/          # CLI help output parsing
├── normalize-chat-sessions/       # Agent session log parsing
├── normalize-session-analysis/    # Session analysis logic
│
├── normalize-surface-syntax/      # Syntax translation (readers/writers)
├── normalize-typegen/             # Type codegen (multiple backends)
├── normalize-openapi/             # OpenAPI client generation
└── xtask/                         # Build automation (publish=false)
```

## CLI Commands (19)

| Command | Description |
|---------|-------------|
| `view` | View directory/file/symbol structure |
| `edit` | Structural code modifications |
| `history` | Shadow git edit history |
| `analyze` | Codebase analysis (21 subcommands) |
| `text-search` | Fast ripgrep-based text search |
| `facts` | Manage code facts (symbols, imports, calls) |
| `rules` | Manage and run analysis rules |
| `init` | Initialize normalize in a directory |
| `daemon` | Background process management |
| `grammars` | Tree-sitter grammar management |
| `update` | Self-update |
| `sessions` | Agent session log analysis |
| `package` | Package management (info, tree, audit) |
| `tools` | External tools (lint, test) |
| `serve` | Server protocols (MCP, HTTP, LSP) |
| `generate` | Code generation (client, types, cli-snapshot) |
| `aliases` | List filter aliases |
| `context` | Show directory context (.context.md files) |
| `translate` | Translate code between languages |

## Data Flow

1. **Parse**: Tree-sitter grammars parse source into ASTs
2. **Extract**: Language trait implementations extract symbols, imports, calls
3. **Store**: Facts stored in SQLite (`.normalize/facts.db`)
4. **Analyze**: Rules (syntax + Datalog) run against extracted facts
5. **Present**: Results formatted via OutputFormatter trait

## Key Patterns

- **Language trait**: Required methods, 98 implementations — syntax/AST extraction
- **LocalDeps trait**: Default methods, ~10 implementations — filesystem/package discovery
- **Ecosystem trait**: Package management (cargo, npm, pip, go, etc.)
- **OutputFormatter trait**: `format_text()` + `format_pretty()` for consistent output
- **Runtime dispatch**: Traits + registry for open-ended extensibility
- **Index-optional**: All commands work without facts DB (graceful degradation)
