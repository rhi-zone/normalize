# Architecture Decisions

Key architectural decisions and their rationale.

## Language Choice: Pure Rust

**Decision**: Moss is implemented entirely in Rust.

### Why Rust?

- **Performance**: Parallel indexing with rayon for large codebases (100k+ files)
- **Tree-sitter native**: First-class tree-sitter integration
- **Single binary**: No runtime dependencies, easy distribution
- **Memory safety**: No GC pauses during indexing

### Crate Structure

```
crates/
├── moss/              # Core library + CLI
├── moss-languages/    # 98 language definitions
├── moss-packages/     # Package ecosystem support
├── moss-tools/        # MCP tool generation
├── moss-derive/       # Proc macros
├── moss-jsonschema/   # Schema generation
└── moss-openapi/      # OpenAPI generation
```

## Dynamic Grammar Loading

**Decision**: Load tree-sitter grammars from external `.so` files.

### Why?

- **Build time**: Bundling 98 grammars bloats compile time
- **Binary size**: Grammars add ~142MB uncompressed
- **User extensibility**: Users can add custom grammars

### Loading Order

1. `MOSS_GRAMMAR_PATH` environment variable
2. `~/.config/moss/grammars/`
3. Built-in fallback (if compiled with grammar features)

## Lua for Workflows

**Decision**: Use Lua (LuaJIT via mlua) for workflow scripting.

### Why Not TOML/YAML?

Once you need conditionals (`if is_dirty() then commit() end`), you're fighting the format. We tried TOML first and deleted ~1500 lines.

### Why Lua?

- ~200KB runtime, extremely fast
- Simple syntax: `view("foo.rs")` vs TOML's `view: foo.rs`
- Battle-tested: nginx, redis, neovim all use Lua
- Full language when needed: loops, functions, error handling

## Index-Optional Design

**Decision**: All commands work without the index (with graceful degradation).

### Fallback Behavior

| Feature | With Index | Without Index |
|---------|------------|---------------|
| Symbol search | SQLite query | Filesystem walk + parsing |
| Health metrics | Cached stats | Real-time file scan |
| Path resolution | Index lookup | Glob patterns |

### Configuration

```toml
[index]
enabled = true  # Set to false to disable indexing entirely
```

## Command Naming: `text-search` Not `grep`

**Decision**: Use `moss text-search` for text pattern matching instead of `moss grep`.

### Why Not `grep`?

1. **AI agent confusion**: LLMs like Claude (especially Opus 4.5) conflate `moss grep` with unix grep syntax. They constantly try `moss grep pattern file` (unix style) instead of `moss text-search pattern` (our style).

2. **Mental model conflict**: Unix grep has 50+ years of muscle memory. Our command uses ripgrep internally but has different semantics (no positional file args, `--only` instead of file patterns). Fighting the unix grep mental model wastes tokens and causes errors.

3. **Semantic expectations**: In the AI era, "search" and "find" imply semantic/vector search. `text-search` explicitly signals regex-based text matching.

### Why Not `search` or `find`?

Those names should be reserved for future semantic search features (embeddings, vector similarity). `text-search` is explicit about the mechanism.

### Config Section

The config section is `[text-search]` to match the command name.

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
| moss-languages | `Language` | Language support (98 built-in) |
| moss-cli-parser | `CliFormat` | CLI help parsing |
| moss-sessions | `LogFormat` | Agent session log parsing |
| moss-tools | `Tool`, `TestRunner` | Tool/runner adapters |
| moss-packages | `Ecosystem` | Package manager support |
| moss-jsonschema | `JsonSchemaGenerator` | Type generation |
| moss-openapi | `OpenApiClientGenerator` | API client generation |

### When to Use Compile-Time Dispatch (Feature Flags)

Use this pattern when:
- The set of implementations is fixed (known at compile time)
- Dependencies are heavy (tree-sitter grammars, large codegen templates)
- No use case for runtime registration
- Consumer knows what they need when compiling

```toml
[features]
default = ["read-typescript", "write-lua"]
read-typescript = ["tree-sitter", "arborium-typescript"]
write-lua = []
```

**Crates using feature flags:**

| Crate | Features | Rationale |
|-------|----------|-----------|
| moss-typegen | Backend selection (ts, rust, python, etc.) | Backends known at compile time, no runtime extensibility needed |
| moss-surface-syntax | Readers/writers (read-typescript, write-lua) | Tree-sitter grammars are heavy; consumers know which languages they need |
| moss-rules | Optional linting backends | Heavy optional dependencies |

### Key Insight

The distinction is about **who decides what's available**:
- **Runtime dispatch**: The running program decides (user can register custom implementations)
- **Compile-time dispatch**: The build decides (developer picks features in Cargo.toml)

Both patterns support single-crate design. Runtime dispatch doesn't require splitting crates (traits + registry in one crate). Compile-time dispatch doesn't require separate crates either (feature flags in one crate).

## Allowlist Conventions

Two mechanisms for allowing/ignoring findings, used for different cases:

### Global Allowlist Files (`.moss/*-allow`)

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

For **single-location code findings** (syntax rules):

```rust
// moss-allow: rust/unwrap-in-impl - input validated above
let value = result.unwrap();
```

Inline comments make sense here because:
- The finding is about a specific piece of code
- Comments survive refactoring (move with the code)
- The reason is visible at the location
- Familiar pattern (clippy `#[allow()]`, ESLint `// eslint-disable`)

### Config-Based Patterns

For **file pattern exclusions** on syntax rules:

```toml
[analyze.rules."rust/unwrap-in-impl"]
allow = ["**/tests/**", "src/bin/**"]
```

This is a third option for "don't lint these files at all" - coarser than inline comments, finer than disabling the rule entirely.

### Summary

| Finding type | Mechanism | Example |
|--------------|-----------|---------|
| Whole-file property | `.moss/*-allow` | Large files, hotspots |
| Cross-location relationship | `.moss/*-allow` | Duplicate functions |
| Code pattern (file exclusion) | Config `allow` patterns | Skip tests for unwrap rule |
| Code pattern (specific instance) | Inline comment | Allow this one unwrap |
