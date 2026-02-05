# Lint Architecture

Design sketch for normalize's multi-tier linting system.

## Philosophy: Insights by Default

Unlike ArchUnit (write tests) or CodeQL (write queries), normalize provides **useful architecture insights out of the box**. No configuration required.

```bash
$ normalize analyze architecture
# Immediately shows: circular deps, coupling metrics, orphan modules, etc.
```

Rules are for **enforcement and customization**, not the baseline experience. Users should get value on first run.

## Motivation

Mature languages have mature linters (clippy, eslint, ruff). But most of the ~98 languages normalize supports don't. The goal isn't to compete with clippy—it's to raise the floor for underserved languages.

More importantly: **no good cross-language architecture analysis tool exists**. ArchUnit is Java-only. NDepend is .NET-only. dependency-cruiser is JS-only. normalize works across 98 languages with the same analysis.

normalize-languages already extracts symbols, imports, exports, calls, and complexity for all supported languages. Wiring this into a rule system gives Gleam/Zig/Nim/Odin users analysis they wouldn't otherwise have.

## Default Insights (No Configuration)

`normalize analyze architecture` should surface these automatically:

### Structural Issues
| Insight | Why it matters |
|---------|---------------|
| Circular dependencies | Always problematic, hard to test/refactor |
| Orphan modules | Dead code, or missing integration |
| Hub modules | Single point of failure, bottleneck |

### Coupling Metrics
| Metric | What it shows |
|--------|--------------|
| Fan-in (afferent) | How many modules depend on this one |
| Fan-out (efferent) | How many modules this one depends on |
| Instability | efferent / (efferent + afferent) - 0=stable, 1=unstable |
| Cross-imports | A↔B bidirectional imports (tight coupling) |

### Module Health
| Insight | What it shows |
|---------|--------------|
| Deep import chains | A→B→C→D→E - long coupling paths |
| Boundary violations | Detected layers (cli/, core/, services/) with wrong-direction imports |
| Large modules | Too many symbols - candidate for splitting |

### Example Output

```
$ normalize analyze architecture

Circular Dependencies (2):
  src/a.rs → src/b.rs → src/c.rs → src/a.rs
  src/handlers/ ↔ src/services/

Coupling Hotspots:
  src/core/config.rs    fan-in: 47  fan-out: 3   instability: 0.06 (stable)
  src/app.rs            fan-in: 2   fan-out: 23  instability: 0.92 (unstable)

Cross-Module Coupling:
  handlers/ ↔ services/:  12 bidirectional imports
  cli/ → core/:           8 imports (one-way ✓)

Potential Issues:
  src/legacy/old_parser.rs - orphan (never imported)
  src/utils/helpers.rs - high fan-in (31), changes here are risky
```

All of this works **without any rules**. Rules add enforcement:
- "Fail CI if circular deps exist"
- "services/ cannot import cli/"
- "instability > 0.8 for core/ is error"

## Use Cases

### Tier 1: Pure Syntax (syntax-rules, exists today)

AST pattern matching via tree-sitter queries. No semantic understanding.

| Rule | Languages | Query |
|------|-----------|-------|
| Debug print statements | All | `(call_expression function: "println")` |
| TODO/FIXME comments | All | `(comment) @c (#match? @c "TODO")` |
| Hardcoded secrets | All | `(string) @s (#match? @s "password=")` |
| Tuple returns | Rust, Python, TS | `(return_statement (tuple))` |

**Value**: Fast, no index needed, works on single files. Good for CI pre-commit hooks.

### Tier 2: Index Queries (index-rules, new)

Pattern matching over semantic data: symbols, imports, calls, complexity.

| Rule | What it catches | Why syntax can't |
|------|-----------------|------------------|
| Unused imports | `import foo` where `foo` never appears in calls/symbols | Needs cross-reference analysis |
| Missing exports | Public API references unexported symbol | Needs export + symbol data |
| High-complexity hotspots | Functions with complexity > N called from > M places | Needs complexity + call graph |
| Deprecated API usage | Calls to functions marked `@deprecated` | Needs symbol metadata |
| Import from banned module | `import { x } from "banned-pkg"` | Could be syntax, but semantic is more precise |
| Orphan symbols | Defined but never called/referenced | Needs full reference analysis |

**Value**: Same analysis depth across all 98 languages. Gleam gets what TypeScript gets.

### Tier 3: Imperative Analysis (normalize-lint, new)

For checks that can't be expressed as pattern matching:

| Rule | Why it needs imperative logic |
|------|-------------------------------|
| Circular dependencies | Requires transitive closure / cycle detection |
| Layering violations | "module A should never transitively depend on B" |
| Dead code elimination | Reachability from entry points |
| API stability | Comparing two index snapshots |
| Custom project rules | "services/ cannot import from cli/" |

**Value**: Escape hatch for complex analysis. Lua scripting for user-defined rules.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     User Rules                          │
│  .scm files (syntax + index)    .lua files (imperative) │
└─────────────────────┬───────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────┐
│                  Query Engine                           │
│         Unified SCM query executor                      │
└──────────┬─────────────────────────────┬────────────────┘
           │                             │
┌──────────▼──────────┐     ┌────────────▼────────────────┐
│   syntax-rules      │     │      index-rules            │
│                     │     │                             │
│  TreeCursor over    │     │  TreeCursor over            │
│  tree-sitter AST    │     │  IndexNode adapter          │
└─────────────────────┘     └─────────────────────────────┘
                                         │
                            ┌────────────▼────────────────┐
                            │   Flat Index Storage        │
                            │  Vec<Symbol>, Vec<Import>,  │
                            │  Vec<Call>, etc.            │
                            └─────────────────────────────┘
```

## Index Tree Projection

The flat index is queried via a tree-shaped interface:

```
(index
  (module path: "src/lib.rs"
    (symbol name: "Config" kind: "struct" exported: true complexity: 5
      (symbol name: "new" kind: "function" complexity: 12))
    (symbol name: "helper" kind: "function" exported: false complexity: 3)
    (import source: "std::collections"
      (name "HashMap"))
    (call target: "helper" line: 45)
    (call target: "HashMap::new" line: 52)))
```

Query examples:

```scm
; High-complexity exported functions
(module
  (symbol kind: "function" exported: true @fn)
  (#gt? @fn.complexity 15))

; Imports from specific module
(module path: @path
  (import source: "unsafe_module"))

; Functions that are defined but never called
; (This is where scm hits limits - need to assert absence)
```

## The Negation Problem

SCM is pattern matching—it finds what exists, not what's missing. Rules like "unused imports" need to assert absence.

Options:
1. **Post-process**: Query returns all imports + all references, Rust code computes the diff
2. **Extended predicates**: `(#not-exists? (call target: @import.name))`
3. **Hybrid**: Simple presence checks in scm, absence checks in normalize-lint

Recommendation: Start with option 1. Keep scm pure, handle negation in Rust. If patterns emerge, consider option 2.

## Crate Structure

```
normalize-syntax-rules   # Exists. AST patterns via scm.
normalize-index-rules    # New. Index patterns via scm.
normalize-lint           # New. Imperative analysis via Lua.
normalize-lint-engine    # New. Shared query executor, reporting, config.
```

## Implementation Decision: Ascent + AOT Compilation

After evaluating Rust Datalog options (Datafrog, Crepe, Ascent, DDlog), the decision is:

**Use Ascent for all Datalog rules, with AOT compilation for user rules.**

Why Ascent:
- State-of-the-art optimizations (index generation, semi-naïve evaluation)
- Lattice support for future data flow analysis
- Clean Rust integration (call Rust from rules, call rules from Rust)
- Not "locked into DSL" - predicates are arbitrary Rust functions

Why not an interpreter:
- All performant Rust Datalog implementations are compile-time
- Building an interpreter that matches Ascent's optimizations is months of work
- AOT with caching gives same developer experience with better performance

### User Rule Workflow

```
User writes rules (.dl files)
         ↓
normalize compile-rules  (generates Rust, compiles via rustc)
         ↓
~/.config/normalize/rules.so  (cached dylib)
         ↓
normalize lint  (loads dylib, runs fast)
```

Rules recompile only when changed. First run is slow (compilation), subsequent runs are fast.

Tradeoff: users need Rust toolchain. Acceptable for users writing Datalog rules.

### Rust Integration

Rules can call Rust and vice versa:

```rust
ascent! {
    relation symbol(PathBuf, String, SymbolKind);
    relation high_complexity(PathBuf, String);

    // Pure Datalog for recursion
    high_complexity(f, s) <--
        symbol(f, s, SymbolKind::Function),
        // Escape to Rust for complex logic
        complexity_exceeds(&f, &s, 15);
}

fn complexity_exceeds(file: &Path, sym: &str, threshold: u32) -> bool {
    // Access index, do arbitrary computation
}
```

## Differentiation from CodeQL

| | CodeQL | normalize |
|---|--------|-----------|
| Extraction | Deep (types, data flow, taint) | Shallow (symbols, imports, calls) |
| Languages | ~12 with dedicated extractors | ~98 via tree-sitter |
| Focus | Security vulnerabilities | Structural/architectural analysis |
| Queries | "Does user input reach SQL?" | "What depends on what?" |

normalize is the lightweight, broad, structural analysis tool. CodeQL is the heavy, deep, security tool. Different jobs.

**Future**: Deep analysis (types, data flow, taint) is on the backlog. Per-language effort, but tractable. Start with TS, Python, Rust.

## Open Questions

1. **Rule naming**: How do rules from different tiers compose? Same namespace or prefixed?
2. **Severity inheritance**: If syntax-rules and index-rules both flag the same line, how to dedupe?
3. **Incremental**: Can index-rules run incrementally on changed files, or always full index?
4. **Error messages**: How to provide fix suggestions without language-specific knowledge?
5. **Dylib loading**: Platform differences (`.so`, `.dylib`, `.dll`), versioning, ABI stability

## Next Steps

1. Add Ascent as dependency, spike a simple rule over current index
2. Design the index → Ascent relation mapping
3. Implement 3 proof-of-concept rules: unused imports, circular deps, high-complexity hotspots
4. Build `normalize compile-rules` command with caching
