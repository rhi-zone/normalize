# Architecture Decisions

Key architectural decisions and their rationale.

## Language Choice: Pure Rust

**Decision**: Normalize is implemented entirely in Rust.

### Why Rust?

- **Performance**: Parallel indexing with rayon for large codebases (100k+ files)
- **Tree-sitter native**: First-class tree-sitter integration
- **Single binary**: No runtime dependencies, easy distribution
- **Memory safety**: No GC pauses during indexing

### Crate Structure

**Decision**: Prefer single crates with modules over multiple crates. Split only when necessary.

**When to split into a separate crate:**
- **Different consumers**: The code is used by external crates independently (e.g., `normalize-derive` for proc macros)
- **Different domains**: The code represents a distinct, self-contained domain (e.g., `normalize-languages` vs `normalize-packages`)
- **Compile time**: The code significantly impacts compile time when changed (isolate it)

**When to keep in one crate:**
- **Small implementations**: Each module is ~100-1000 lines (readers, writers, backends)
- **Shared core**: All modules depend on shared types (e.g., an IR)
- **Single consumer**: Only one crate uses this code
- **Coherent domain**: The modules are conceptually part of one thing

**Rule of thumb**: If you'd have 3+ crates each under 500 lines, keep them as modules in one crate. Crate overhead (Cargo.toml, lib.rs, versioning, publishing) isn't worth it for small code.

```
crates/
├── normalize/                         # Main CLI binary
├── normalize-core/                    # Core types and utilities
├── normalize-derive/                  # Proc macros (must be separate)
├── normalize-output/                  # OutputFormatter trait, format flags
│
├── normalize-languages/               # Language trait (98 implementations)
├── normalize-language-meta/           # Language metadata (extensions, names)
├── normalize-grammars/                # Tree-sitter grammar loading (publish=false)
│

├── normalize-edit/                    # Edit command logic
├── normalize-shadow/                  # Shadow git (edit history)
├── normalize-filter/                  # --exclude/--only filtering
├── normalize-path-resolve/            # Path resolution utilities
│
├── normalize-facts/                   # Fact extraction + SQLite storage
├── normalize-facts-core/              # Fact data types (Symbol, Import, etc.)
├── normalize-facts-rules-api/         # Stable ABI for rule plugins (abi_stable)
├── normalize-facts-rules-builtins/    # Built-in fact rules (cdylib)
├── normalize-facts-rules-interpret/   # Interpreted Datalog rules
│
├── normalize-syntax-rules/            # Tree-sitter query rules (.scm)
├── normalize-rules-loader/            # Rule loading infrastructure
│
├── normalize-deps/                    # Dependency analysis
├── normalize-local-deps/              # LocalDeps trait (import resolution)
├── normalize-ecosystems/              # Ecosystem trait (cargo, npm, pip)
├── normalize-package-index/           # PackageIndex trait (apt, brew)
│
├── normalize-tools/                   # External tool orchestration
├── normalize-cli-parser/              # CLI help output parsing
├── normalize-chat-sessions/           # Agent session log parsing
├── normalize-session-analysis/        # Session analysis logic
│
├── normalize-surface-syntax/          # Syntax translation (readers/writers)
├── normalize-typegen/                 # Type codegen (multiple backends)
├── normalize-openapi/                 # OpenAPI client generation
└── xtask/                             # Build automation (publish=false)
```

## Dynamic Grammar Loading

**Decision**: Load tree-sitter grammars from external `.so` files.

### Why?

- **Build time**: Bundling 98 grammars bloats compile time
- **Binary size**: Grammars add ~142MB uncompressed
- **User extensibility**: Users can add custom grammars

### Loading Order

1. `NORMALIZE_GRAMMAR_PATH` environment variable
2. `~/.config/normalize/grammars/`
3. Built-in fallback (if compiled with grammar features)

## .scm Query Files over Rust for Node Classification

**Decision**: When classifying AST nodes by concept (functions, calls, complexity contributors, scopes, etc.), use tree-sitter `.scm` query files — not Rust methods returning `&'static [&'static str]`.

### The rule

If you're writing `fn complexity_nodes() -> &'static [&'static str] { &["if_expression", "match_arm", ...] }`, that's a tree-sitter query expressed as Rust data. Write it as a `.scm` file instead:

```scheme
; complexity.scm
(if_expression) @complexity
(match_arm) @complexity
```

### When .scm is right

- Node identification: "which nodes are functions / calls / complexity contributors?"
- Answerable with a tree-sitter query and a capture name
- The result is "a set of matching nodes" — no structured field extraction needed

### When Rust is right

- Structured data extraction: getting the name, parameters, visibility *from* an identified node
- Logic that depends on the node's children or fields (e.g. `extract_function`, `extract_imports`)
- Anything requiring decisions, not just matching

### Implication for the Language trait

Methods like `complexity_nodes()`, `nesting_nodes()`, `control_flow_kinds()`, `scope_creating_kinds()`, and the missing call extraction all belong in `.scm` files, not the trait. The trait should contain only extraction methods and metadata that requires Rust logic.

`locals.scm` (`*.locals.scm`) is the established pattern. New query files follow the same convention: `*.complexity.scm`, `*.calls.scm`, etc., loaded via `GrammarLoader`.

## Index Design

**Decision**: The index (SQLite via libsql) is an implementation detail — commands build it on demand, cache it in `.normalize/`, and invalidate on file changes.

The index is **not optional for cross-file features**. Call graphs, import resolution, cross-file references, and Datalog rules all require persistent indexed data. Commands that depend on these hard-fail without an index and prompt the user to run `normalize structure rebuild`.

Single-file operations (view, edit, within-file analysis) work without any index.

### Future: ephemeral index for small repos

For small repos, the index could be built in `/tmp` keyed by repo root + mtime, making it fully transparent. Not yet implemented.

### Configuration

```toml
[facts]
# enabled = true  # Set to false to disable indexing entirely
```

## Command Naming: `grep` (Reversed from `text-search`)

**Decision**: Use `normalize grep` for text pattern matching.

**History**: Originally named `text-search` to avoid AI agents confusing `normalize grep` with unix grep syntax (positional file args, BRE/ERE regex). This was reversed because:

1. **`text-search` didn't help** — agents confused `text-search` just as much as `grep` (wrong syntax, wrong regex dialect). The rename didn't solve the problem it was designed to solve.
2. **`grep` is universally understood** — the name instantly communicates "regex text search" to both humans and AI agents
3. **`text-search` caused its own confusion** — agents would sometimes try `normalize search` or `normalize find` instead
4. **Consistency** — short, unix-inspired names (`grep`, `view`, `edit`) fit the CLI's style better than compound names

### Note on regex syntax

`normalize grep` uses **ripgrep regex**, not unix grep regex. `|` for alternation (not `\|`). Use `(a|b)` grouping. No BRE/ERE distinction. This is documented in CLAUDE.md to prevent agent confusion.

## Local Model Memory Budget

For future local LLM/embedding integration:

| Model | Params | FP16 RAM |
|-------|--------|----------|
| all-MiniLM-L6-v2 | 33M | 65MB |
| distilbart-cnn | 139M | 280MB |
| T5-small | 60M | 120MB |
| T5-base | 220M | 440MB |

### Recommendations

1. Default to smallest viable model
2. Lazy loading (don't load until first use)
3. Graceful degradation (fall back to extractive methods if OOM)
4. Consider INT8 quantization (~4x memory reduction)

### Pre-summarization Tiers

1. **Zero-cost**: Title, headings, metadata extraction
2. **Extractive**: TextRank, TF-IDF (no NN needed)
3. **Small NN**: Embeddings, abstractive summary
4. **LLM**: Only when simpler methods insufficient

## Extensibility Patterns: Runtime vs Compile-Time

**Decision**: Use runtime dispatch (traits + registry) when users need extensibility. Use compile-time dispatch (feature flags) when the set is fixed and dependencies are heavy.

### When to Use Runtime Dispatch (Traits + Registry)

Use this pattern when:
- Users might register custom implementations at runtime
- The feature set is open-ended (new languages, tools, package managers)
- Implementations are lightweight (~200 lines, no heavy deps)

```rust
use std::sync::{OnceLock, RwLock};

// Global registry
static FORMATS: RwLock<Vec<&'static dyn MyTrait>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

// Public registration function
pub fn register(item: &'static dyn MyTrait) {
    FORMATS.write().unwrap().push(item);
}

// Lazy initialization of built-ins
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        let mut formats = FORMATS.write().unwrap();
        formats.push(&BuiltinA);
        formats.push(&BuiltinB);
    });
}

// Lookup functions call init_builtin() first
pub fn get(name: &str) -> Option<&'static dyn MyTrait> {
    init_builtin();
    FORMATS.read().unwrap().iter().find(|f| f.name() == name).copied()
}
```

**Crates using runtime dispatch:**

| Crate | Trait | Purpose |
|-------|-------|---------|
| normalize-languages | `Language` | Language support (98 built-in) |
| normalize-cli-parser | `CliFormat` | CLI help parsing |
| normalize-chat-sessions | `LogFormat` | Agent session log parsing |
| normalize-tools | `Tool`, `TestRunner` | Tool/runner adapters |
| normalize-ecosystems | `Ecosystem` | Project dependency management |
| normalize-package-index | `PackageIndex` | Distro/registry index ingestion |
| normalize-typegen | `Backend` | Type/validator codegen |
| normalize-openapi | `OpenApiClientGenerator` | API client generation |

### When to Use Compile-Time Dispatch (Feature Flags)

Use this pattern when:
- The set of implementations is fixed (known at compile time)
- Dependencies are heavy (tree-sitter grammars, large codegen templates)
- No use case for runtime registration
- Consumer knows what they need when compiling

```toml
[features]
default = ["backend-typescript", "backend-rust"]
backend-typescript = []
backend-rust = []
```

**Crates using feature flags only:**

| Crate | Features | Rationale |
|-------|----------|-----------|
| normalize-typegen | Backend selection (ts, rust, python, etc.) | Backends known at compile time, no runtime extensibility needed |
| normalize-syntax-rules | Optional linting backends | Heavy optional dependencies |

### Hybrid: Traits + Feature Flags

Some crates need both:
- **Traits + registry**: For user extensibility (custom implementations)
- **Feature flags**: For consumer customizability (include only what you need)

Use this when users should be able to add custom implementations, AND consumers should be able to opt out of built-ins they don't need.

```toml
[features]
default = ["read-typescript", "write-lua"]
read-typescript = ["tree-sitter", "arborium-typescript"]  # Heavy dep
write-lua = []
```

```rust
// Trait for extensibility
pub trait Reader: Send + Sync {
    fn language(&self) -> &str;
    fn read(&self, source: &str) -> Result<Program, ReadError>;
}

// Registry for custom implementations
pub fn register_reader(reader: &'static dyn Reader) { ... }

// Built-in readers behind feature flags (heavy deps)
#[cfg(feature = "read-typescript")]
pub use typescript::TypeScriptReader;
```

**Crates using hybrid approach:**

| Crate | Pattern | Rationale |
|-------|---------|-----------|
| normalize-surface-syntax | `Reader`/`Writer` traits + feature flags | Users can add languages; built-in readers need tree-sitter grammars |

### Key Insight

The distinction is about **who decides what's available**:
- **Runtime dispatch**: The running program decides (user can register custom implementations)
- **Compile-time dispatch**: The build decides (developer picks features in Cargo.toml)
- **Hybrid**: Build decides which built-ins to include; runtime allows additions

Both patterns support single-crate design. Runtime dispatch doesn't require splitting crates (traits + registry in one crate). Compile-time dispatch doesn't require separate crates either (feature flags in one crate).

## Allowlist Conventions

Two mechanisms for allowing/ignoring findings, used for different cases:

### Global Allowlist Files (`.normalize/*-allow`)

For **file-level or cross-location findings**:

| File | Purpose |
|------|---------|
| `hotspots-allow` | Files with high git churn (metadata about file history) |
| `duplicate-functions-allow` | Pairs of intentionally similar functions |
| `duplicate-types-allow` | Pairs of intentionally similar types |
| `large-files-allow` | Files allowed to exceed size threshold |

These can't be inline comments because:
- They're about whole files, not specific lines
- They're about relationships between multiple locations
- They're metadata about files (churn, size), not code

### Inline Comments

For **single-location code findings**, with namespaced prefixes per rule system:

```rust
// normalize-syntax-allow: rust/unwrap-in-impl - input validated above
let value = result.unwrap();
```

```python
# normalize-facts-allow: god-file - this file is intentionally large
def one_of_many_functions(): ...
```

Syntax rules check the finding's line and the line above. Fact rules check the
first 10 lines of the file (fact diagnostics are file-level, not line-level).

Inline comments make sense here because:
- The finding is about a specific piece of code (or file, for facts)
- Comments survive refactoring (move with the code)
- The reason is visible at the location
- Familiar pattern (clippy `#[allow()]`, ESLint `// eslint-disable`)

### Config-Based Patterns

For **file pattern exclusions** on syntax rules:

```toml
[rules."rust/unwrap-in-impl"]
allow = ["**/tests/**", "src/bin/**"]
```

This is a third option for "don't lint these files at all" - coarser than inline comments, finer than disabling the rule entirely.

### Summary

| Finding type | Mechanism | Example |
|--------------|-----------|---------|
| Whole-file property | `.normalize/*-allow` | Large files, hotspots |
| Cross-location relationship | `.normalize/*-allow` | Duplicate functions |
| Code pattern (file exclusion) | Config `allow` patterns | Skip tests for unwrap rule |
| Syntax finding (specific instance) | `normalize-syntax-allow:` comment | Allow this one unwrap |
| Fact finding (specific file) | `normalize-facts-allow:` comment | Allow this god-file |

## Facts & Rules Naming

**Decision**: Use "facts" terminology instead of "index" for extracted code metadata.

### Why "facts"?

- **Domain-agnostic**: normalize isn't limited to programming languages - could analyze configs, docs, data formats
- **Datalog alignment**: The rules engine uses Datalog (via Ascent), which operates on "facts" with "rules"
- **Precision**: "index" is vague (search index? database index?); "facts" describes what we extract - assertions about code/data
- **Extensible**: As we add type extraction, data flow, etc., they're all just more facts

### Crate naming

```
normalize-facts-core               # data types only (SymbolKind, Symbol, Import, FlatSymbol, etc.)
normalize-facts                    # full library: extraction + storage + queries (depends on core)
├── normalize-facts-rules-api      # stable ABI for rule plugins
└── normalize-facts-rules-builtins # default analysis rules
```

### Plugin architecture

All rules (builtin and user) compile to dylibs via `abi_stable`:
- Uniform infrastructure - no special-casing between builtin vs user rules
- Builtins ship pre-compiled, users compile theirs with `normalize facts compile`
- Rule packs can be shared and version-controlled independently
- Hot-swappable without recompiling the main binary

## Multi-Repo Report Shape

**Decision**: Extend single-repo report types with an optional `repos` field rather than returning a separate `MultiRepoReport<T>` wrapper type.

### Context

`normalize analyze hotspots/ownership/coupling` support `--repos DIR` to run across all git repos under a directory. This required a design decision on return type shape.

### Options considered

1. **Separate commands** (`hotspots-multi`) — clean types but duplicates the command surface
2. **Untagged union** — `HotspotsOutput::Single(T) | HotspotsOutput::Multi(MultiRepoReport<T>)` — one command but callers must handle two shapes
3. **Always wrap in `MultiRepoReport<T>`** — consistent outer shape but forces single-repo callers to unwrap an array
4. **Extend single-repo report** — add `repos: Option<Vec<RepoResult>>` alongside existing top-level fields ✓

### Why option 4

- **Stable top-level shape**: `--jq .files` works identically with or without `--repos`. Options 2 and 3 break this.
- **Semantic correctness**: `MultiRepoReport<T>` means "here are N repos" — a different concept from "a hotspots report that optionally aggregates multiple repos"
- **Caller simplicity**: No conditional unwrapping based on whether `--repos` was passed

### Invariant

The top-level fields of a report are always present and always mean the same thing. `--repos` adds a `.repos` field alongside them with per-repo breakdowns.

## scm Query Files over Rust Node Lists

**Decision**: Replace `Language` trait methods that return `&'static [&'static str]` node-kind lists (e.g. `complexity_nodes()`, `nesting_nodes()`) with `.scm` tree-sitter query files.

### Why

- **Tree-sitter queries are the native abstraction** for matching AST patterns. Node-kind lists are a subset that loses structural information (parent-child relationships, field names, anonymous nodes like `&&`/`||`).
- **User-customizable**: `.scm` files can be overridden via `NORMALIZE_GRAMMAR_PATH` without recompiling.
- **Consistent ecosystem**: Highlights, injections, and locals all use `.scm` files. Complexity and calls should too.
- **Less per-language Rust code**: A generic query walker replaces N language-specific manual tree-walkers.

### Pattern

1. Queries are bundled at compile time via `include_str!` in `grammar_loader.rs` as fallbacks.
2. `GrammarLoader::get_complexity(name)` / `get_calls(name)` check external search paths first, then bundled.
3. The walker compiles the query once per analysis, then runs it on each function node.
4. Captures: `@complexity`/`@nesting` for complexity, `@call`/`@call.qualifier` for calls.

### Migration Status

- `*.complexity.scm`: 12 languages (Rust, Python, Go, JS, TS, TSX, Java, C, C++, Ruby, Kotlin, Swift). Query-based walker runs alongside trait-based fallback.
- `*.calls.scm`: 7 languages (Python, Rust, TS, TSX, JS, Java, Go). Old per-language walkers removed; generic query walker is the only path.
- `complexity_nodes()` / `nesting_nodes()` trait methods preserved for backward compat (used by languages without `.scm` files).
- Future: `scope_creating_kinds()`, `control_flow_kinds()` are candidates for the same migration.

## CLI Command Organization (2026-03-16)

### Context

`normalize analyze` accumulated ~42 subcommands with no guiding principle. Auditing the
existing top-level subcommands revealed the structural problem: every other top-level
subcommand unifies a domain via a trait with multiple implementations (`rules` →
`RuleEngine`, `tools` → tool trait, `syntax` → `Language`). `analyze` had no such trait.

### Decisions

**1. Top-level subcommands must unify a domain via a trait.**
A subcommand is not a namespace for loosely related operations. The test: is there a trait
where each subcommand variant is an implementation? If not, the commands belong elsewhere.

**2. Introduce `normalize rank` — the ranking primitive.**
~80% of `analyze` commands answer "rank this codebase by metric X." The unifying primitive
is `Rankable`: produce an ordered list of (item, score) pairs. Input tier (file-only, git,
index) is an implementation detail, not a user-facing concept. `normalize rank complexity`,
`normalize rank coupling`, etc. The `RankEntry` infrastructure already existed in
`normalize-analyze::ranked`.

**3. `normalize view <target> <subcommand>` — target-first navigation.**
Graph operations (`call_graph`, `trace`, `dependents`, `provenance`) and `--history` are
navigation of a specific named entity: "show me what's connected to / around this thing."
They belong in `view`, not `analyze`. The pattern is `path subcommand` (target first,
operation second): `normalize view src/foo.rs/Bar callers`, `normalize view src/foo.rs/Bar
history`. This is consistent with `view`'s existing target-first convention.

**4. `ViewOutput` enum is a symptom to fix.**
A command with one coherent purpose has one output shape. `ViewOutput` having 9 variants is
a sign that `view` accumulated operations that should be subcommands. Each subcommand on
`view` gets one output shape. The fix is mechanical: convert flag-driven output variants
into proper subcommands.

**5. `analyze` dissolves.**
What remains in `analyze` after extracting `rank` and moving graph/history ops to `view`:
prose summaries (`health`, `summary`, `architecture`), time series (`activity`, `trend-metric`),
and a few other non-ranking commands. These are deferred — no forced unification until the
right primitive is identified. `analyze` shrinks toward zero; it doesn't get a new identity.

### Rejected alternatives

- **Arbitrary graph query interface** (Datalog, Cypher, jq): every tool that achieved this
  either embedded a full query engine or exposed facts for an external tool. Neither is
  lightweight. The canned commands are fine for real use cases; the issue was the set had
  no closure property. Giving `view` a `callers`/`dependents`/etc. subcommand model gives
  the closure property without a query language.
- **`analyze run --pass <...>`** modeled on `rules run --engine`: rejected because `rank`
  already provides the right abstraction for the orchestration case (run multiple metrics,
  get ranked output). The `rules` model works because all engines share input/output shape;
  analysis passes don't.
- **Reorganize `analyze` into sub-services** (graph, quality, structure): reorganizing a
  grab-bag produces a smaller grab-bag. The root cause was no unifying trait, not bad grouping.
