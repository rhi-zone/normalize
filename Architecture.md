# Moss Architecture

High-level architecture for in-context learning. See `docs/` for detailed documentation.

## Core Concept

Moss is a **tooling orchestration layer** with structural awareness. It understands code structure (AST, CFG, dependencies) rather than treating code as raw text.

## Key Components

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI (cli.py)                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Skeleton   │  │     CFG      │  │ Dependencies │      │
│  │  Extractor   │  │   Builder    │  │   Analyzer   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         │                 │                 │               │
│         └─────────────────┼─────────────────┘               │
│                           ▼                                 │
│                   ┌──────────────┐                          │
│                   │    Views     │ (Plugin System)          │
│                   └──────────────┘                          │
│                           │                                 │
│  ┌──────────────┐        │        ┌──────────────┐         │
│  │   Synthesis  │◄───────┴───────►│  Validators  │         │
│  │  Framework   │                 │              │         │
│  └──────────────┘                 └──────────────┘         │
│         │                                 │                 │
│         └─────────────────┬───────────────┘                │
│                           ▼                                 │
│                   ┌──────────────┐                          │
│                   │  Shadow Git  │ (Atomic Commits)         │
│                   └──────────────┘                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Module Organization

```
src/moss/
├── cli.py              # Command-line interface
├── skeleton.py         # AST → function/class signatures
├── cfg.py              # Control flow graph builder
├── dependencies.py     # Import/export analysis
├── plugins/            # View plugins (tree-sitter, markdown, etc.)
├── synthesis/          # Code synthesis framework
│   ├── framework.py    # Main synthesis engine
│   ├── strategies/     # Decomposition strategies
│   └── plugins/        # Generators, validators, libraries
├── validators.py       # Syntax, ruff, pytest validation
├── shadow_git.py       # Git-based atomic operations
├── events.py           # Event bus for component communication
└── memory.py           # Episodic memory for context
```

## Data Flow

1. **Input**: Task description, target file/symbol
2. **Analysis**: Extract skeleton, build CFG, analyze dependencies
3. **Synthesis**: Decompose problem → generate code → validate
4. **Output**: Validated code with atomic git commit

## Key Patterns

### Plugin System
Views are provided by plugins discovered via entry points:
- `moss.plugins` - View plugins (skeleton, cfg, deps)
- `moss.synthesis.generators` - Code generators
- `moss.synthesis.validators` - Code validators

### Synthesis Loop
```
Specification → Decompose → Generate → Validate → (retry if failed) → Solution
```

### Structural Awareness
Code is understood through its structure:
- **Skeleton**: What functions/classes exist, their signatures
- **CFG**: How control flows through functions
- **Dependencies**: What imports what

## Conventions

- **Commits as units of work**: Each commit is a logical change
- **Tests at all levels**: Unit, integration, E2E
- **ruff for linting**: `ruff check` and `ruff format`
- **Type hints**: All public APIs are typed
