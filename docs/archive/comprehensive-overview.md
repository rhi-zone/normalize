# Comprehensive Architecture Overview

This document provides a complete picture of Normalize's architecture, current capabilities, and identified gaps.

## Core Concept: Unified Codebase Tree

Normalize treats the codebase as a single tree where filesystem and AST are levels of the same hierarchy:

```
project/                    # root node
├── src/                    # directory node
│   ├── main.py             # file node
│   │   ├── class Foo       # class node (AST)
│   │   │   └── bar()       # method node (AST)
│   │   └── def helper()    # function node (AST)
```

Uniform addressing with `/`: `src/main.py/Foo/bar` addresses the method `bar` in class `Foo`.

## Three Primitives

Instead of 100+ specialized tools, Normalize provides three composable primitives:

| Primitive | Purpose | Read/Write |
|-----------|---------|------------|
| `view`    | See/find nodes in the tree | Read |
| `edit`    | Modify nodes structurally | Write |
| `analyze` | Compute properties of nodes | Read |

This design minimizes tool selection ambiguity and maximizes composability.

## The Zoom Model

### Current Implementation

The `view` command uses depth-based expansion:

| Depth | Output |
|-------|--------|
| 0 | File/directory name only |
| 1 | Signatures with `...` body placeholder (default) |
| 2 | Full implementation |
| -1 | All levels expanded |

Example at depth 1 (skeleton):
```python
def login(user: str, password: str) -> User:
    """Authenticate user and return session."""
    ...
```

### Current Filters

| Flag | Purpose |
|------|---------|
| `--type <kind>` | Filter by node type: class, function, method |
| `--deps` | Show imports/dependencies |
| `--calls` | Show callers (what calls this?) |
| `--called-by` | Show callees (what does this call?) |

### Gap Analysis vs Gemini Feedback

Gemini identified several "zoom" improvements. Here's the comparison:

#### 1. Uniform Zoom Trap

**Problem:** Depth-based zoom treats all files equally. A shallow `utils.py` gets fully expanded while a deep `MainController.py` stays collapsed.

**Current state:** We have `--type` filtering but not visibility-based filtering.

**Gap:** Need semantic zoom levels:
- Level 1: File paths only
- Level 2: Exported/public symbols only
- Level 3: All top-level symbols
- Level 4: Full implementation

**Suggestion:** Add `--visibility public|all` or `--exported` flag.

#### 2. Signature vs Body (Already Implemented)

**Gemini's suggestion:** Show signatures + docstrings but collapse bodies.

**Current state:** This is exactly what depth=1 does. The skeleton format uses `...` as body placeholder, which Gemini explicitly praised.

**Status:** Done.

#### 3. Fisheye View (Mixed Zoom)

**Problem:** Agent needs high resolution on focus file, low resolution on periphery.

**Gemini's suggestion:** Support mixed zoom:
- Focus file: depth 2 (full)
- Direct imports: depth 1 (signatures)
- Everything else: depth 0 (paths only)

**Current state:** No built-in fisheye. User must make multiple `view` calls.

**Gap:** Add `--fisheye <target>` mode or multi-target syntax.

#### 4. Hoisted Imports

**Problem:** Agent sees `import { validate } from '../utils'` but has to make another call to see what `validate` is.

**Suggestion:** Inject imported symbol signatures inline.

**Current state:** `--deps` shows import statements but not resolved signatures.

**Gap:** Add `--resolve-imports` to inline imported symbol signatures.

#### 5. Ghost Files / Barrel Re-exports

**Problem:** `index.ts` that re-exports looks empty but exposes entire API.

**Current state:** Shows imports in `--deps` but doesn't hoist re-exported symbols.

**Gap:** Detect `export * from` patterns and hoist symbols into tree view.

#### 6. Types-First View

**Gemini's highest-ROI suggestion:** A mode that shows ONLY types/interfaces/signatures, stripping all function bodies.

Use case: Get entire architectural map for ~5k tokens instead of 100k.

**Current state:** depth=1 skeleton gets close but still shows function signatures with `...` bodies.

**Gap:** Add `--types-only` flag that shows:
- Interface definitions
- Type aliases
- Class property types
- Function signatures (no docstrings, no bodies)

## Rust/Python Boundary

### Principle

**Rust = Plumbing** (fast, deterministic, syntax-aware)
**Python = Interface** (LLM, orchestration, UI)

### Current Division

```
┌─────────────────────────────────────────────────────────────┐
│                     Python (src/normalize/)                       │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐│
│  │   CLI    │ │   TUI    │ │   MCP    │ │   Workflows      ││
│  │ (cli.py) │ │(tui.py)  │ │  Server  │ │ (execution/)     ││
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┬─────────┘│
│       │            │            │                │          │
│  ┌────┴────────────┴────────────┴────────────────┴─────┐    │
│  │                    rust_shim.py                      │    │
│  │              (subprocess → Rust binary)              │    │
│  └──────────────────────────┬───────────────────────────┘    │
└─────────────────────────────│────────────────────────────────┘
                              │ JSON
┌─────────────────────────────│────────────────────────────────┐
│                     Rust (crates/)                           │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    normalize-cli/src/                         ││
│  │  main.rs ─┬─► view, edit, analyze (commands)            ││
│  │           ├─► skeleton.rs (AST → signatures)            ││
│  │           ├─► symbols.rs (symbol extraction)            ││
│  │           ├─► index.rs (SQLite call graph)              ││
│  │           └─► path_resolve.rs (fuzzy resolution)        ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    normalize-core/                            ││
│  │  Shared: tree-sitter parsers, Language, SymbolKind      ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Overlap Resolution

| Feature | Rust | Python | When Rust | When Python |
|---------|------|--------|-----------|-------------|
| Skeleton | yes | yes | Large codebase | Python-specific AST |
| Edit | yes (structural) | yes (LLM) | Delete, move, rename | Complex refactors |
| Complexity | yes | yes | Batch analysis | Detailed breakdown |

Goal: Reduce overlap. Python wraps Rust, doesn't reimplement.

## File Organization

### Large Files (Technical Debt)

| File | Lines | Issue |
|------|-------|-------|
| `src/normalize/cli.py` | 5687 | ~40 `cmd_*` functions in one file |
| `src/normalize/normalize_api.py` | 4148 | ~15 API classes in one file |
| `crates/normalize-cli/src/main.rs` | 3655 | Monolithic Rust CLI |
| `src/normalize/tui.py` | 2028 | TUI app + 6 mode classes |

### Recommended Splits

**cli.py → cli/ package:**
```
cli/
├── __init__.py      # Main parser, dispatch
├── analysis.py      # analyze, complexity, health
├── editing.py       # edit, mutate, refactoring
├── generation.py    # gen, synthesize
├── navigation.py    # view, search, context
└── utilities.py     # init, config, status
```

**normalize_api.py → api/ package:**
```
api/
├── __init__.py      # NormalizeAPI facade
├── edit.py          # EditAPI
├── skeleton.py      # SkeletonAPI
├── tree.py          # TreeAPI
├── patterns.py      # PatternsAPI
├── telemetry.py     # TelemetryAPI
└── shadow_git.py    # ShadowGitAPI
```

## Plugin Architecture

Normalize uses Protocol classes for extensibility:

| Plugin Type | Protocol | Location |
|-------------|----------|----------|
| View providers | `ViewPlugin` | `plugins/__init__.py` |
| Linters | `LinterPlugin` | `plugins/linters.py` |
| Code generators | `CodeGenerator` | `synthesis/protocols.py` |
| Validators | `SynthesisValidator` | `synthesis/protocols.py` |
| Log parsers | `LogParser` | `session_analysis.py` |
| LLM providers | `LLMProvider` | `llm/protocol.py` |

Discovery via entry points in `pyproject.toml`.

## Workflow Execution

Recent addition: state machine workflows with composable primitives.

```
workflows/
├── dwim.yaml        # Main agentic workflow
└── ...

execution/
├── primitives.py    # StepResult, ExecutionContext
├── runner.py        # Step execution engine
├── state_machine.py # State transitions, LLM selection
└── parallel.py      # Fork/join execution
```

Features:
- Nested workflows (steps can invoke sub-workflows)
- Parallel states (fork/join)
- LLM-driven state selection
- Modal keybinds in TUI

## Identified Gaps

### High Priority (Blocking Dogfooding)

1. **Types-only view** - Get architectural map cheaply
2. **Fisheye view** - Mixed resolution for focus + context
3. **Resolve imports** - Inline imported signatures

### Medium Priority (Polish)

4. **Visibility filter** - Public vs all symbols
5. **Barrel file hoisting** - Re-export detection
6. **Useless docstring detection** - Skip "Sets the user id" on `setUserId`

### Low Priority (Nice to Have)

7. **Split cli.py** - Housekeeping, not blocking
8. **Split normalize_api.py** - Housekeeping, not blocking
9. **Split main.rs** - Already partially done

## Next Steps

1. Implement `--types-only` flag (highest ROI per Gemini)
2. Implement `--fisheye` or multi-target view syntax
3. Add `--resolve-imports` to inline imported signatures
4. Dogfood with real tasks, log friction points
