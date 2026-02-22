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

## Rule File Format

Rules are self-documenting. Each `.scm` or `.dl` file contains three sections:

1. **TOML frontmatter** — machine-readable metadata
2. **Markdown doc block** — human-readable documentation (optional but encouraged)
3. **Query/program** — the actual rule logic

```scheme
# ---
# id = "rust/unwrap-in-impl"
# severity = "info"
# message = ".unwrap() found - consider using ? or .expect() with context"
# tags = ["safety", "error-handling"]
# languages = ["rust"]
# allow = ["**/tests/**", "**/examples/**"]
# enabled = false
# ---
#
# Unwrap in trait implementations can panic in library code, giving callers
# no way to handle the error. This is especially problematic in library crates
# where panicking is considered rude — callers expect to handle failures.
#
# ## How to fix
#
# Prefer `?` to propagate errors to the caller, or `.expect()` with context
# if you're genuinely certain the value can't be `None`/`Err`.
#
# ## Examples
#
# **Bad:**
# ```rust
# fn process(&self) -> String {
#     self.value.lock().unwrap()  // panics if poisoned
# }
# ```
#
# **Good:**
# ```rust
# fn process(&self) -> Result<String, PoisonError> {
#     Ok(self.value.lock()?.clone())
# }
# ```
#
# ## When to disable
#
# In binary crates or application code where panicking is acceptable,
# or in test helpers where a panic is the desired failure mode.

; tree-sitter query
((call_expression
  function: (field_expression
    field: (field_identifier) @_method)
  (#eq? @_method "unwrap")) @match)
```

The doc block uses `#` comment lines containing markdown. It stays in the same file as the rule — documentation never drifts from the implementation. `rules show <id>` renders it; `rules list --expand` shows the first paragraph truncated.

Aim for the same standard as Clippy, ESLint, or Ruff rule pages: rationale, bad/good examples, remediation, when to disable. All accessible offline.

### Tier 1: Pure Syntax (syntax-rules, exists today)

AST pattern matching via tree-sitter queries. No semantic understanding.

| Rule | Languages | Query |
|------|-----------|-------|
| Debug print statements | All | `(call_expression function: "println")` |
| TODO/FIXME comments | All | `(comment) @c (#match? @c "TODO")` |
| Hardcoded secrets | All | `(string) @s (#match? @s "password=")` |
| Tuple returns | Rust, Python, TS | `(return_statement (tuple))` |

**Value**: Fast, no index needed, works on single files. Good for CI pre-commit hooks.

### Tier 2: Fact Rules (Datalog over the index)

Datalog rules over extracted facts: symbols, imports, calls. Uses the ascent-interpreter for zero-compilation user rules, with compiled dylib path for performance-critical packs.

| Rule | What it catches | Why syntax can't |
|------|-----------------|------------------|
| Circular dependencies | A imports B imports C imports A (transitive) | Needs recursive closure |
| Orphan modules | File never imported by anything | Needs cross-file join |
| Hub modules | Module imported by >N others | Needs aggregation over imports |
| God files | File with >N functions | Needs counting over symbols |
| Dead API surface | Public function unreachable from entry points | Needs visibility + transitive calls |
| Architecture violations | `domain/` transitively imports `infra/` | Needs recursive closure + path predicates |

**Value**: Same analysis depth across all 98 languages. Gleam gets what TypeScript gets. Users write `.dl` files — no Rust toolchain needed for the interpreted path.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                     User Rules                           │
│   .scm files (syntax)              .dl files (facts)     │
└──────────┬──────────────────────────────┬────────────────┘
           │                              │
┌──────────▼──────────┐     ┌─────────────▼───────────────┐
│   syntax-rules      │     │      fact-rules             │
│                     │     │                             │
│  TreeCursor over    │     │  Datalog engine over        │
│  tree-sitter AST    │     │  extracted Relations        │
│                     │     │                             │
│  Per-file, no index │     │  Two execution paths:       │
│                     │     │  • Interpreted (ascent-eval) │
│                     │     │  • Compiled (dylib + ascent) │
└─────────────────────┘     └─────────────────────────────┘
                                         │
                            ┌─────────────▼───────────────┐
                            │   Facts Index (SQLite)      │
                            │  symbols, imports, calls,   │
                            │  files, type_methods        │
                            └─────────────────────────────┘
```

## Crate Structure

```
normalize-syntax-rules         # AST pattern matching via tree-sitter queries (.scm)
normalize-facts-core           # Data types (Symbol, SymbolKind, Visibility, Import, etc.)
normalize-facts                # Extraction + SQLite storage (Extractor, FileIndex)
normalize-facts-rules-api      # Stable ABI for compiled rule packs (Ascent + abi_stable)
normalize-facts-rules-builtins # Compiled builtin rules (cdylib)
```

The main `normalize` crate contains the interpreted Datalog engine (`interpret.rs`) and
the builtin `.dl` rules (`builtin_dl/`).

## Two Execution Paths for Fact Rules

### Path 1: Interpreted (ascent-interpreter)

```
User writes .dl file with TOML frontmatter
         ↓
normalize facts check  (parses + interprets via ascent-eval)
         ↓
Diagnostics (warnings/errors)
```

- **No compilation** — rules run immediately
- Supports: recursion, negation, aggregation, lattices, string predicates
- Uses ascent-interpreter (pterror's fork): ascent-syntax → ascent-ir → ascent-eval
- Builtin rules are embedded via `include_str!` and loaded automatically
- User rules: `~/.config/normalize/rules/*.dl` and `.normalize/rules/*.dl`

### Path 2: Compiled (dylib via abi_stable)

```
User writes Ascent rules in Rust
         ↓
cargo build  (compiles to .so/.dylib)
         ↓
normalize facts rules --pack <path>  (loads dylib, runs fast)
```

- **Full Ascent power** — arbitrary Rust in predicates, type safety, optimizations
- Useful for: performance-critical rules, rules that need filesystem/network access
- Stable ABI via `abi_stable` — dylibs work across normalize versions

### When to use which

| | Interpreted | Compiled |
|---|---|---|
| Setup | Zero (write .dl, run) | Rust toolchain + cargo build |
| Performance | Fine for <100k facts | Needed for large codebases |
| Expressiveness | Pure Datalog only | Datalog + arbitrary Rust |
| Distribution | Copy .dl file | Share .so/.dylib |
| Iteration speed | Instant | Compile cycle |

For most users and rules, interpreted is sufficient. Compiled is the escape hatch.

## Current Fact Set (What Datalog Rules See)

```
symbol(file: String, name: String, kind: String, line: u32)
import(from_file: String, to_module: String, name: String)
call(caller_file: String, caller_name: String, callee_name: String, line: u32)
visibility(file: String, name: String, vis: String)
attribute(file: String, name: String, attr: String)
parent(file: String, child_name: String, parent_name: String)
qualifier(caller_file: String, caller_name: String, callee_name: String, qualifier: String)
symbol_range(file: String, name: String, start_line: u32, end_line: u32)
implements(file: String, name: String, interface: String)
is_impl(file: String, name: String)
type_method(file: String, type_name: String, method_name: String)
```

### What's extracted but NOT exposed

| Field | In Symbol | In SQLite | In Datalog Relations |
|-------|-----------|-----------|---------------------|
| `name` | yes | yes | yes |
| `kind` | yes | yes | yes |
| `start_line` | yes | yes | yes (as `line`) |
| `end_line` | yes | yes | yes (via `symbol_range`) |
| `parent` | yes (via `children`) | yes | yes |
| `visibility` | yes | yes | yes |
| `attributes` | yes | yes | yes |
| `signature` | yes | **no** | **no** |
| `implements` | yes | yes | yes |
| `is_interface_impl` | yes | yes | yes (via `is_impl`) |
| `docstring` | yes | **no** | **no** |

## Fact Enrichment Roadmap

The path to making fact rules genuinely useful. Ordered by impact and implementation effort.

### Phase 1: Expose what we already extract (DONE)

All four relations wired into Relations and available in Datalog.

- `visibility(file, name, vis)` — Public/Private/Protected/Internal for all 98 languages
- `attribute(file, name, attr)` — decorators, annotations, macros
- `parent(file, child_name, parent_name)` — symbol nesting hierarchy
- `qualifier(caller_file, caller_name, callee_name, qualifier)` — call qualifiers (self, module, etc.)

### Phase 2: Persist currently-discarded Symbol fields (DONE)

- `symbol_range(file, name, start_line, end_line)` — symbol span for line-count rules
- `implements(file, name, interface)` — interface/trait implementation
- `is_impl(file, name)` — symbol is a trait/interface implementation
- `type_method(file, type_name, method_name)` — method signatures on types

### Builtin rules using Phase 1+2 relations (DONE)

Four new builtin `.dl` rules exercise these relations (all `enabled = false` by default):

- **god-class** — Type with >20 methods. Uses `parent` + `symbol`.
- **long-function** — Function body >100 lines. Uses `symbol_range`.
- **dead-api** — Public function never called from another file. Uses `visibility` + `call`.
- **missing-impl** — Class implements interface but missing required methods. Uses `implements` + `type_method` + `parent`.

### Phase 3: New extraction (requires tree-sitter work)

These don't exist yet in the `Language` trait.

- **`export(file, name)`** — distinguish exports from definitions (some languages conflate them)
- **`type_annotation(file, name, type_str)`** — parameter/return types for type-based rules
- **`data_flow(file, from_sym, to_sym)`** — assignment/parameter flow within a function

Phase 3 is significant per-language work and should be driven by specific rule needs.

## What Normalize Should and Shouldn't Implement

The existence of eslint, oxlint, biome, ruff, rust-analyzer, go vet, clangd etc. raises an obvious question: should normalize reimplement checks that already exist in per-language tools?

**No — and the reason is architectural, not just "avoid duplication."**

Per-language linters operate at the **semantic/syntactic layer**: type-aware checks, dataflow analysis, ownership rules, language-specific idioms. They have deep knowledge of one language and sophisticated analysis to match.

Normalize operates at the **structural/architectural layer**: import graphs, module coupling, export surfaces, dependency topology. This layer is:
- **Cross-language by nature** — the same circular-dependency rule applies to Python, Rust, Go, and Gleam
- **Not covered by per-language tools** — eslint cannot find a circular dependency between a JS module and a Python module
- **Expressible over already-extracted facts** — no new extraction needed

The right test for any new check: *can this only be expressed with cross-file or cross-language joins?* If yes, it belongs in normalize. If it's fundamentally about one file's semantics, the per-language tool does it better.

### Layer Boundary in Practice

| Check | Tool | Why |
|-------|------|-----|
| Circular deps between modules | normalize | Requires transitive join across all files |
| Coupling metrics (fan-in/fan-out) | normalize | Requires aggregation over entire import graph |
| Architecture violations (layer A → layer B) | normalize | Requires path predicates + recursive closure |
| Cross-language dead exports | normalize | Requires joining across language boundaries |
| Rust borrow checker violations | rust-analyzer | Requires type + ownership analysis |
| JS type errors | typescript/eslint | Requires type inference |
| Python async pitfalls | ruff | Language-specific idiom knowledge |
| C++ Rule of Three/Five | clang-tidy | Already covered (`cppcoreguidelines-special-member-functions`) |

### Dogfooding is Not the Justification

Running normalize on normalize is valuable — but the value is in finding *architectural* issues (layering violations between crates, coupling hotspots, circular deps), not in replacing clippy. Clippy still runs; normalize adds the structural layer that clippy can't express.

The goal is not a single tool that does everything. The goal is: normalize does the cross-language structural layer extremely well, and composes cleanly with per-language tools for the rest.

## Differentiation from CodeQL

| | CodeQL | normalize |
|---|--------|-----------|
| Extraction | Deep (types, data flow, taint) | Broad (symbols, imports, calls across 98 langs) |
| Languages | ~12 with dedicated extractors | ~98 via tree-sitter |
| Focus | Security vulnerabilities | Structural/architectural analysis |
| Queries | "Does user input reach SQL?" | "What depends on what?" |
| Rule authoring | QL (custom language) | Datalog (.dl) or Rust (dylib) |
| Setup | GitHub-hosted or heavy local install | Single binary |

normalize is the lightweight, broad, structural analysis tool. CodeQL is the heavy, deep, security tool. Different jobs.

## Rule Tags

Tags are the unit of user intent. Rules are the unit of implementation. These are different things and shouldn't be collapsed.

### Built-in tags

Defined in .scm/.dl rule frontmatter:

```toml
# ---
# id = "js/console-log"
# tags = ["debug-print"]
# languages = ["javascript", "typescript"]
# ---
```

Built-in tags group rules by concept across languages. `--tag debug-print` enables all debug print rules for all languages at once — the user thinks in concepts, not language-specific rule IDs.

### User-defined tags in normalize.toml

Two mechanisms, both additive:

**Tag groups** — define a named set of rules/tags (top-down):

```toml
[rule-tags]
pre-commit = ["debug-print", "cleanup"]
strict = ["pre-commit", "dead-api", "circular-deps"]  # tags can reference other tags
```

**Per-rule tags** — add a rule to a concept (bottom-up):

```toml
[[rules]]
id = "js/console-log"
severity = "error"
tags = ["pre-commit"]  # appends to built-in tags, does not replace
```

Both resolve the same way at query time. Resolution is recursive expand-and-deduplicate.

If a user-defined tag name matches a built-in tag, they union — same name means same concept. A user adding `debug-print = ["my-custom-logger"]` in `[rule-tags]` extends the built-in `debug-print` set rather than replacing it.

### CLI surface

```bash
# Run by concept, not by rule ID
normalize rules run --tag pre-commit        # fast, all languages
normalize rules run --tag debug-print --language rust  # filters compose

# Enable/disable by concept or by ID
normalize rules enable debug-print          # enables all rules tagged debug-print
normalize rules disable js/console-log      # disables one specific rule
normalize rules enable debug-print --dry-run  # preview what would change

# Discover
normalize rules list                         # all rules
normalize rules list --tag debug-print       # rules matching a tag
normalize rules list --language rust --enabled  # enabled Rust rules
normalize rules tags                         # all tags (builtin + user-defined)
normalize rules tags --show-rules            # expand each tag to its member rules
normalize rules tags --tag debug-print       # show what's in one tag
```

### Tag display and color

In `--pretty` output, tags are color-coded using deterministic hashing: the tag name is hashed to an index into a curated palette of visually distinct colors. The same tag always renders in the same color across invocations, with no configuration needed.

The palette is chosen deliberately — ~12 colors readable on both light and dark backgrounds, avoiding red and yellow which are reserved for error/warning severity indicators. Info uses a neutral color (e.g. dim white/blue).

`rules list` shows severity and tags on each row, collapsed by default:

```
RULE                     LEVEL   TAGS
rust/println-debug       info    debug-print  cleanup
js/console-log           error   debug-print  pre-commit
circular-deps            warn    architecture
dead-api                 warn    architecture  strict
```

`--expand` shows options and the first line of documentation per rule:

```
RULE                     LEVEL   TAGS
rust/println-debug       info    debug-print  cleanup
  allow:   **/tests/**  **/examples/**  **/main.rs
  message: println!/print! found - consider using tracing or log crate
  docs:    Println in library code leaks debug output to callers...

js/console-log           error   debug-print  pre-commit
  allow:   **/tests/**  **/*.test.*
  fix:     (auto-fixable)
  docs:    console.log left in production code exposes internal state...
```

`rules show <id>` renders the full documentation for one rule — rationale, examples, remediation, when to disable.

`rules tags --show-rules` expands by tag:

```
debug-print  [builtin]   js/console-log  python/print-debug  rust/println-debug  go/fmt-print
cleanup      [builtin]   ...
pre-commit   [user]      debug-print  cleanup  rust/unwrap-in-impl
strict       [user]      pre-commit  dead-api  circular-deps
```

Tag colors are consistent across both views — `debug-print` is always the same color whether it appears as a column in `rules list` or as a header in `rules tags`.

Color applies in `--pretty` mode only. `--compact` and `--json` output is never colored.

## Inline Suppression

Two namespaced comment prefixes for per-diagnostic suppression:

```rust
// normalize-syntax-allow: rust/unwrap-in-impl - validated above
let value = result.unwrap();
```

```python
# normalize-facts-allow: god-file - this file is intentionally large
```

Syntax rules check the finding's line and the line above it. Fact rules check the first
10 lines of the file (since fact diagnostics are file-level, not line-level).

## Open Questions

1. ~~**Rule naming**: How do rules from different tiers compose?~~ **Resolved**: Separate namespaces — `normalize-syntax-allow:` vs `normalize-facts-allow:`
2. **Incremental**: Can fact rules run incrementally on changed files, or always full index?
3. **Error messages**: How to provide fix suggestions without language-specific knowledge?
4. ~~**Dylib loading**: Platform differences, ABI stability~~ **Resolved**: `abi_stable` crate handles this
