# Normalize Roadmap

See `CHANGELOG.md` for completed work. See `docs/` for design docs.

## Priorities

Production-grade refactoring across all ~98 languages. Goal: rename, find-references,
extract, inline, move — correct, without LSPs, without false positives.

1. **locals.scm for remaining languages** — Write `locals.scm` for 77+ languages still missing it.
   Each must be accompanied by fixture tests before it counts as done (unverified = worse than none).
   - See: [Semantic Refactoring Infrastructure](#semantic-refactoring-infrastructure)

2. **Comprehensive language fixtures** (long-term, nix flake verification)
   - See: [Semantic Refactoring Infrastructure](#semantic-refactoring-infrastructure)

## Session Queue (2026-03-07)

Ordered by impact × tractability. Pick from top.

1. ~~**Eliminate `cmd_*` layer** — `commands/analyze/` done (dead wrappers deleted).~~
   **Remaining:** `commands/rules.rs` still has i32-returning `cmd_*` functions called from
   `service/rules.rs`. Convert to `Result<T, String>` to unblock `?` propagation.
   See: [Eliminate cmd_* layer](#eliminate-cmd_-layer--move-logic-into-service-methods)

2. ~~**Wire `tags.scm` into symbol extraction**~~ — DONE. `collect_symbols_from_tags()` is the
   primary path; Language trait has only 3 required methods. No node-kind lists remain.

3. ~~**Remaining info/warning noise (batch-fix)**~~ — DONE. Production code is clean (only
   fixture files have violations, which is correct).

4. **Language coverage: `.scm` query files**
   - `*.calls.scm` — every language without it silently produces zero call graph data
   - `*.complexity.scm` — missing for all languages that still use `complexity_nodes()` in Rust
   - `*.types.scm` — missing for all typed languages (C#, Java, Kotlin, Swift, C, C++, Scala, Go)
   Target: parity with `locals.scm` coverage (65 languages). One `.scm` + fixture test per language.

5. **Feature-gate CLI behind `cli` feature** — library consumers shouldn't pull in clap.
   Add standalone CLIs to `normalize-facts`, `normalize-filter`, `normalize-syntax-rules`.
   See: [Feature-gate CLI](#feature-gate-cli-behind-cli-feature-workspace-wide)

3. **normalize as LSP server** (stretch)
   - `textDocument/references`, `textDocument/rename`, `textDocument/definition` backed by normalize
   - Proxy mode: `normalize serve lsp --proxy 'rust-analyzer'`
   - See: [Semantic Refactoring Infrastructure](#semantic-refactoring-infrastructure)

## Immediate Fixes

### ~~Incremental caching for `normalize analyze check --summary`~~ DONE

Cache at `.normalize/cache/summary-freshness.json`, keyed by HEAD commit hash. Warm run:
~3s (was ~20s). `git status` always re-run (cheap); `git log` skipped when HEAD unchanged.

### ~~Bootstrap all SUMMARY.md files~~ DONE

272 SUMMARY.md files written (parallel Sonnet agents, one per crate group + fixture dirs).
Severity re-escalated to `error`; hook blocks commits when SUMMARY.md is too stale.
Staleness condition: `(commits_since_update + has_uncommitted) > threshold` (configurable,
default 10). Single uncommitted change alone no longer blocks commits.

### ~~Audit info/hint rule noise~~ PARTIAL (pre-commit --no-fail removed)

Pre-commit now enforces zero error-severity violations. Resolved (9ae3b496):
- `no-grammar-loader-new`: 2 production fixes; allow pattern for test modules needing `add_path()`
- `rust/numeric-type-annotation`: 15 violations fixed; allow pattern for test fixtures
- `rust/tuple-return`: Lowered from error → warning (54 violations remain — see below)

**Remaining info noise (219 issues):** Not yet addressed. Current breakdown:
- `rust/chained-if-let`: 122 violations (info)
- `rust/unnecessary-type-alias`: 36 violations (info)
- `rust/unnecessary-let`: 32 violations (info)
- `no-todo-comment`: 17 violations (info)
- Other: 12 violations across 6 rules

Strategy: batch-fix by rule. Priority order:
1. `rust/unnecessary-type-alias` ×36 — straightforward inline
2. `rust/unnecessary-let` ×32 — straightforward inline
3. `no-todo-comment` ×17 — review for false positives first
4. `rust/chained-if-let` ×122 — run `normalize rules run --fix` (auto-fix exists)

**rust/tuple-return (54 warnings):** Need `ByteRange { start, end }` struct or similar in
normalize-languages for `container_body()` return type. Also affects normalize-manifest and
normalize-package-index parsers. Plan: create shared `normalize-ranges` or reuse existing type.

The exclude patterns in `[analyze]` still don't apply to rules. Adding a global
`[rules.exclude]` section analogous to `[analyze].exclude` would clean up fixture violations.

### Guided rule setup — DONE (`normalize init --setup`)

Implemented as `normalize init --setup` (interactive terminal wizard).

**What it does:** Runs all rules against the codebase, groups violations by rule (sorted by
count desc), and walks the user through each — showing rule metadata and up to 5 example
violations — then prompting [e]nable / [d]isable / [s]kip / [q]uit. Persists decisions via
`normalize rules enable/disable`.

**LLM-stateful flow** (already works via existing CLI):
1. `normalize rules list --json` to enumerate candidates
2. `normalize rules run --rule <id>` to review top violations per rule
3. `normalize rules enable/disable <id>` to update config

**Remaining:** Review default-enabled rule set. Several rules generate high noise; guided
setup is the cure. Also: `normalize init --setup` currently only covers rules — extend to
other project-level decisions as they emerge (e.g., exclude patterns, SUMMARY.md enforcement).

### ~~Actionable output for all diagnostic commands~~ DONE (9ae3b496)

`DiagnosticsReport` now has `hints: Vec<String>`. Service methods populate hints
based on context and output mode:
- `rules run` (non-pretty, non-sarif): "Run with --pretty for detailed view" + "--fix to auto-fix"
- `analyze check` (non-pretty): "Run `normalize analyze check [flags] --pretty` for detailed view"

Remaining gaps:
- `rules run --engine sarif` could show which SARIF tools had errors (not done)
- Hints only appear in text mode. Pretty mode shows them too but they're redundant there.
  Could suppress hints in pretty mode (hints are most useful in the compact/default mode).

### ~~Failing skeleton tests (4 tests)~~ FIXED
Root causes:
- **Go/Ruby:** `collect_symbols_from_tags` had a sanity check that bailed when `@definition.method` had no enclosing container — but Go receiver methods and Ruby standalone methods are legitimately top-level. Removed the check.
- **Java:** `java.tags.scm` was missing `enum_declaration` pattern. Added `@definition.enum`.
- **Markdown:** No `markdown.tags.scm` existed. Created it with `(section (atx_heading (inline) @name)) @definition.heading`.
- Also added `definition.enum`/`definition.heading` to `tags_capture_to_kind` and `Enum`/`Heading` to `is_container_kind`.

### ~~LSP diagnostics for all rule engines~~ IMPLEMENTED (basic)
Implemented on-save diagnostics with debounce (500ms). Design doc: `docs/design/lsp-diagnostics.md`.
Future improvements:
- Per-file syntax rules (only re-run on the saved file, not the whole workspace)
- Incremental fact rules (currently rebuilds full index each run)
- Configurable debounce interval
- Progress reporting during long runs

## Next Up

### ~~Audit `rust/unwrap-in-impl`~~ — COMPLETE

Rule enabled at `severity = "warning"` (2026-03). All 1,608 violations resolved:
production code inline-suppressed with reasons or properly fixed; test-only files
allowed via config. See `.normalize/config.toml` for the full allow list.

**Remaining follow-up: eliminate `cmd_*` layer** (see below).

**Audited crates** (production code clean, test-only unwraps allowed in config):
- `normalize-path-resolve` — complete
- `normalize-ecosystems` — complete
- `normalize-facts` — complete
- `normalize-manifest` — complete (248 → 0 production violations; 3 fixes applied:
  nuget.rs if-let refactor, ocaml.rs peek-then-next inline suppress, setup_py.rs bytes-index refactor;
  244 test-only unwraps allowed via config glob)
- `normalize-typegen` — complete (35 → 0 production violations; all inline-suppressed with reasons:
  RwLock poison, infallible iterator after len check, char case conversion always yields a char)
- `normalize-cli-parser` — complete (30 → 0 production violations; all inline-suppressed with reasons:
  RwLock poison, f64 partial_cmp NaN-is-detector-bug, literal Regex::new compile-time guaranteed)
- `normalize-languages` — complete (25 → 0 production violations; all inline-suppressed with reasons:
  RwLock poison, ASCII-quote chars().nth byte==char, non-empty guard before last())
- `normalize-edit` — complete (16 violations; all in inline #[cfg(test)] blocks — allowed in config)
- `normalize-view` — complete (12 → 2 production violations inline-suppressed: is_none guard before unwrap,
  len==1 guard before iter().next())

### Eliminate `cmd_*` layer — move logic into service methods

`main()` calls `NormalizeService::cli_run()` (server-less), not `cmd_*` directly. The
`cmd_*` functions are a vestigial layer: they return `i32` exit codes, which means errors
can't be propagated with `?` and must either be printed inline or swallowed. This produces
panics instead of clean error messages for things like `Runtime::new()` failures.

The right fix is to delete `cmd_*` entirely and move logic directly into service methods:
- Service methods return `Result<T, String>` — server-less handles exit codes and error
  formatting automatically
- `Runtime::new()?` works naturally (mapped to `String` error)
- No more `eprintln + return 1` scattered across command files

**Scope:** All files under `crates/normalize/src/commands/` that expose a `cmd_*` function.
Each service method in `service/*.rs` calling `cmd_*` should inline the logic instead.

**Note:** Some `cmd_*` functions have long parameter lists because they predate the service
layer. As logic moves in, parameters become `&self` fields (pretty, compact, root, etc.)
already available on `NormalizeService`.

### Feature-gate CLI behind `cli` feature (workspace-wide)

Every crate should be usable both as a library and as a standalone CLI tool. Library consumers shouldn't pull in clap; CLI users get a binary. This is a workspace-wide convention, not a one-off.

**Sub-crates that should get standalone CLIs:**
- `normalize-facts` — `normalize-facts index`, `normalize-facts check`
- `normalize-filter` — pipe-friendly filtering tool
- `normalize-syntax-rules` — standalone rule runner
- Others as needed — each crate's CLI exposes its core functionality directly

### Language trait: migrate *_kinds() methods to .scm query files

The `Language` trait has several methods that return `&'static [&'static str]` — lists of
tree-sitter node type names. These are tree-sitter queries expressed as Rust data instead of
using the query system. See `docs/architecture-decisions.md` ("scm Query Files over Rust").

**CRITICAL: Flesh out language coverage** — current counts are abysmal:
- `*.complexity.scm`: Missing: all others that have `complexity_nodes()` in their Language impl.
  Every language that has a grammar should have one.
- `*.calls.scm`: Missing every other language with function calls — a language without calls.scm
  produces zero call graph data — silently broken.
- `*.types.scm`: Missing every typed language — c_sharp, java, kotlin, swift, c, cpp, scala, go, etc.

For each: write the `.scm`, add to `bundled_*_query()` in `grammar_loader.rs`, verify with a
fixture test. Target: coverage matching `locals.scm` (65 languages).

- [ ] **Wire tags.scm into symbol extraction — replace Language trait node-classification
  methods entirely.** `tags.scm` makes the following trait methods redundant:
  `container_kinds()`, `function_kinds()`, `type_kinds()`, `public_symbol_kinds()`,
  `extract_function()`, `extract_container()`, `extract_type()`. Replace the extractor's
  node-kind dispatch with a generic query runner: load `get_tags(grammar)`, run it, derive
  `Symbol` from each `@name.definition.*` capture (kind from capture name, lines from
  parent node, name from capture text). The Language trait shrinks from ~25 methods to
  ~8 genuinely semantic ones:
  - Keep: `extract_docstring()`, `get_visibility()`, `is_public()`, `is_test_symbol()`,
    `test_file_globs()`, `format_import()`, `signature_suffix()`, `embedded_content()`
  - Delete: everything that just encodes node type names as `&'static [&'static str]`
  This is the single highest-leverage refactor remaining in the codebase.
- [ ] **`*.imports.scm`** — import/require statement extraction. Would replace `import_kinds()`
  + `extract_imports()` across ~98 language impls. Captures: `@import.path`, `@import.name`.
- [ ] Implement calls.scm for all languages that have call extraction
- [ ] Replace per-language inline walkers in `symbols.rs` with a generic walker over `call_node_kinds()`

### Type relationship extraction (facts index) — HIGH PRIORITY

Currently `analyze graph --on types` works but only uses shallow symbol-level relationships (impl/extends). Deeper type edges are needed for meaningful structural analysis.

**Needed edges (beyond current impl/extends):**
- **Field types**: struct A has a field of type B → A depends on B
- **Signature types**: fn takes A / returns B → caller's type depends on A, B
- **Type aliases**: `type Foo = Bar` → Foo depends on Bar
- **Generic bounds**: `T: SomeTrait` → depends on SomeTrait

**Implementation:**
- New `type_refs` table: `(file, source_type, target_type, kind, line)` where kind ∈ {field, param, return, impl, alias, bound}
- Extract via `Language` trait method (new required method or `LocalDeps` extension — TBD)
- Start with Rust + TypeScript/Python (richest type systems in the current grammar set)
- Once extracted, `analyze graph --on types` will use these edges alongside existing impl/extends relationships

### Git Analysis Enhancements

**Remaining:**
- [ ] **Cross-repo hotspots**: aggregate churn/complexity/coupling across repos. Which repo has the most tech debt?

**Commands:**
- [ ] **Run commands across repos**: `normalize --repos ~/git/org/ tools lint`, `normalize --repos ~/git/org/ analyze hotspots`. Discover projects, run in parallel, aggregate output.

## Remaining Work

- Namespace-qualified lookups: `normalize view std::vector`, `normalize view com.example.Foo`
  - Requires language-specific namespace semantics - low priority
- Shadow worktree: true shadow-first mode (edit in shadow, then apply)
  - Current: --shadow flag works, but not default for all edits
  - Zero user interruption (user can edit while agent tests in background)

### Configuration System
Sections: `[daemon]`, `[index]`, `[aliases]`, `[view]`, `[analyze]`, `[grep]`, `[pretty]`, `[serve]`

Adding a new section (3 places):
1. Define `XxxConfig` struct with `#[derive(Merge)]` + `XxxArgs` with `#[derive(Args)]` in command module
2. Add field to NormalizeConfig
3. Add `run(args, json)` function that loads config and merges

Candidates: `[workflow]` (directory, auto-run)

### Trait-Based Extensibility
All trait-based crates follow the normalize-languages pattern for extensibility:
- Global registry with `register()` function for user implementations
- Built-ins initialized lazily via `init_builtin()` + `OnceLock`
- No feature gates (implementations are small, not worth the complexity)

Pattern: traits are the extensibility mechanism. Users implement traits in their own code, register at runtime. normalize CLI can add Lua bindings at application layer for scripting.

### CLI Internal Consolidation

**Top-level command level issues (low priority):**
- `history` is at the wrong level: shadow edit history is a feature of `edit`, not a
  standalone concept. Should be `normalize edit history [list|diff|status|tree|prune]`.
- `analyze rules` is redundant with top-level `normalize rules run`. Should be removed from AnalyzeService.
- `context` could be `normalize view context [path]` but semantics differ slightly (content-only vs prepend). Low priority.
- `aliases` is a cross-cutting utility. Too small for top-level but has no clear parent. Low priority.

### Rust Redesign Candidates
- Rules engine: consider semgrep/ruff integration instead of custom
- Plugin system: Rust trait-based plugins or external tool orchestration

### Main Crate Size (`normalize`, 52k lines)

`normalize analyze size -r crates/normalize/src` breakdown (2026-03):

| Area | Lines | % |
|---|---|---|
| `commands/analyze/` | 21,296 | 41% |
| `service/` | 4,372 | 8% |
| `commands/view/` | 4,071 | 8% |
| `commands/sessions/` | 3,383 | 6.5% |
| `serve/` | 1,485 | 3% |
| `tree.rs` | 1,497 | 3% |
| `analyze/` (non-cmd) | 1,372 | 2.6% |
| `skeleton.rs` | 627 | 1.2% |

**Don't bulk-extract `commands/analyze/` as a unit.** The right approach is to extract
*generally useful functionality* into domain crates — algorithms that the LSP, external
tools, or other commands would want. Pure "compute + format for one command" stays.

**Secondary targets (lower priority):**
- `serve/` (LSP + HTTP + MCP, 1.5k) → `normalize-serve`
- `src/analyze/` (1.4k, pure computation) → belongs in `normalize-architecture` or `normalize-facts`
- `commands/sessions/` (3.4k) — circular dep risk, needs care

## Backlog

### normalize-manifest: eval-backed parsing (`eval` feature gate)

Heuristic parsers in `normalize-manifest` cover ~95% of real-world files but fail on
code-as-config formats (Gemfile, mix.exs, build.gradle, flake.nix, Package.swift) where
variables and conditionals can't be resolved from text alone.

**Design**: feature-gate eval capability inside `normalize-manifest` itself (not a
separate crate, not in `normalize-local-deps`). Eval is about parsing fidelity, not
ecosystem discovery.

```rust
// always available — heuristic
pub fn parse_manifest(filename, content) -> Option<ParsedManifest>

// feature = "eval" — tries subprocess first, falls back to parse_manifest automatically
pub fn parse_manifest_eval(filename, content, root: &Path, policy: EvalPolicy) -> Option<ParsedManifest>
```

`parse_manifest_eval` degradation order:
1. Official dump command (runtime-native, safe) → perfect results
2. Wrapper script executed in the runtime → declared deps with variables resolved
3. `parse_manifest` heuristic fallback → always returns something

`EvalPolicy`: `IfAvailable` (try, fall back silently) | `Required` (error if runtime absent)

**Official dump commands** (safe, no arbitrary code exec):
- `cargo metadata --format-version 1` (Rust)
- `go list -json -m all` (Go)
- `npm ls --json` (Node)
- `swift package dump-package` (Swift — already outputs JSON)
- `bundle list --format json` (Ruby, Bundler ≥ 2.4)
- `mix deps.tree` (Elixir — needs shaping into ParsedManifest)

**Wrapper scripts** (executes project code — opt-in only):
- Gemfile: `ruby -r bundler -e 'puts Bundler.definition.dependencies.to_json'`
- mix.exs: elixir wrapper that loads Mix.Project and captures deps config
- flake.nix: `nix eval .#inputs --json`
- build.gradle: inject a task that dumps resolved configurations

**Tree-sitter middle tier** (no execution, better than heuristic):
- Worth considering for code-as-config formats as a tier 1.5 between heuristic and eval
- Handles multiline expressions, strips comments, correct block boundaries
- Still can't resolve runtime variables, but dramatically fewer false negatives
- Belongs in same feature gate or a separate `tree-sitter` feature in normalize-manifest

### Analyze Command Consolidation — HIGH PRIORITY

See `docs/design/analyze-consolidation.md` for full design (axis decomposition, phased plan).

**The CLI is too big.** ~38 subcommands under `analyze` (down from 50 after coverage/churn/duplicates/patterns merges; now grouped via `#[server(groups(...))]` in `--help`). Users can't hold this in working memory. Grouping helps discoverability but doesn't reduce the surface enough.

**Current state (2026-03):**
- `--help` output is now grouped into 8 sections (code, modules, repo, graph, git, test, security, diff) via server-less `#[server(groups(...))]`
- `normalize-analyze` crate provides shared rank infrastructure: `Entity` trait, `Scored<E>`, `rank_pipeline`, `rank_and_truncate`, `truncate_path`
- 16 commands migrated to shared rank infrastructure (complexity, length, density + 13 via rank_and_truncate)
- Output formats remain per-command (too heterogeneous to unify into one generic formatter — each has different columns, stats, grouping)

**Phase 2 — Merge obvious families:**
- [ ] **2a. `health`**: needs design — `health` is default command, param signatures diverge
- [ ] **2c. `density`**: needs design — `uniqueness` has 8 extra params

**Phase 3 — Further consolidation (needs design):**
- [ ] `duplicates` + `fragments`: collapse remaining similarity commands (duplicate-types still separate, fragments absorbed patterns)
- [ ] `deps`: collapse 10 commands (imports, depth-map, surface, layering, architecture, call-graph, callers, callees, trace, impact)
- [x] `docs` → unified `check` command: `check-refs`, `stale-docs`, `check-examples` → `normalize analyze check [--refs] [--stale] [--examples]`. Shared `DiagnosticsReport` in `normalize-output::diagnostics`. `docs` (coverage) stays separate (metric/rank). See `docs/design/rules-unification.md`
- [ ] `git`: collapse 5 commands (ownership, contributors, activity, repo-coupling, cross-repo-health) — all git/repo-centric analysis
- [ ] Cross-cutting `--trend` and `--diff <ref>` modifiers on any scoring command

**Design pressure:** ~43 commands after Phase 2 is still too spread out. Phase 3 must happen. The goal is a surface small enough that a user can hold it in working memory — not just "fewer than 49".

**Enum-return "unifications" — DONE:**

`CoverageOutput` and `CouplingOutput` were enum wrappers — not real unification. No shared shape existed between inner report structs. Split back to separate commands:
- [x] `CoverageOutput` → `test-ratio`, `test-gaps`, `budget` (3 separate service methods)
- [x] `CouplingOutput` → `coupling`, `coupling-clusters`, `hotspots` (3 separate service methods)

### Rules Unification & `facts` → `structure` Rename

See `docs/design/rules-unification.md` for full design.

**Three threads:**

1. **Unified diagnostic type** — DONE. `Issue` + `DiagnosticsReport` in `normalize-output::diagnostics`. Conversion functions `finding_to_issue` and `abi_diagnostic_to_issue` in `normalize::diagnostic_convert`. Ad-hoc checks (`BrokenRef`, `MissingExample`, `StaleDoc`, `StaleSummary`) already converted. Remaining: `SecurityFinding` → `DiagnosticsReport`, wire native checks as `--engine native`.

4. **Unify rule engine config** — `syntax-rules` has a config system (`RulesConfig`, per-rule overrides, severity mapping). The other engines (native, fact, future SARIF) have none. Extract a shared `normalize-rules-config` crate (or extend `normalize-output`) with a unified config schema: rule IDs, severity overrides, enable/disable, per-directory excludes. All engines consult this at run time; `normalize rules run` passes it down.

5. **SARIF passthrough engine** (`--engine sarif`) — accepts a list of external tool commands that emit SARIF output. Runs them with configurable parallelism (default: 8). Parses each tool's stdout as SARIF 2.1.0 and merges into `DiagnosticsReport`. Enables wrapping ESLint, clippy, semgrep, etc. without per-tool adapters. Config lives in `[rules.sarif]` in normalize.toml:
   ```toml
   [[rules.sarif.tools]]
   name = "eslint"
   command = ["npx", "eslint", "--format", "json", "{root}"]
   [[rules.sarif.tools]]
   name = "semgrep"
   command = ["semgrep", "--sarif", "{root}"]
   ```
   Tools that emit JSON (not SARIF) need a `format = "json"` adapter — stretch goal.

6. **`normalize analyze check` help text is scuff** — "Use flags to run specific checks only" appears in the doc comment and gets repeated as-is. Rewrite the doc comment as a single clean sentence; the individual `--flag` help strings carry the per-flag detail. No need to enumerate flags in the top-level description.

2. **Lift `rules` to top level** — DONE. `normalize rules` is now top-level. `--type` → `--engine`. `normalize facts rules` and `normalize facts check` removed. `normalize syntax` retains only `ast` and `query`.

3. **Rename `facts` → `structure`** — DONE. `normalize structure rebuild/stats/files/packages`.

### Semantic Refactoring Infrastructure

Goal: production-grade refactoring (rename, find-references, extract, inline, move) across
all ~98 supported languages, without relying on LSPs. Strategy: tree-sitter locals queries
for within-file scope/reference resolution, facts index for cross-file import/export graph.

**Known locals.scm scope engine limitation:**
- Nested destructuring (e.g. `{ a: { b } }` in parameters) requires recursive queries which
  tree-sitter does not support. One level of object/array destructuring IS covered for JS/TS/TSX.
  Fixing deeper nesting would require engine-level recursion (walk into nested patterns).

**Write locals.scm for remaining languages** (scope/reference queries — not type inference,
just: which declaration does this identifier refer to?):
- Each locals.scm must be accompanied by fixtures before it counts as done.
  An unverified locals.scm is worse than none — it produces silent wrong renames.
- [ ] High-value tier (most-used, well-understood grammars): rust, python, go, java, c, cpp,
      c_sharp, kotlin, ruby, php, bash, zig, dart, elixir, erlang, haskell (already done), clojure
- [ ] Medium tier: julia, nim, crystal, d, groovy, perl, fortran, cobol, pascal, prolog, racket,
      scheme, common_lisp, janet, fennel, haxe, actionscript, coffeescript, purescript, reason,
      solidity, move, wren, v
- [ ] Config/DSL tier (simpler scoping rules): toml, yaml, json (trivially no locals), css, scss,
      less, graphql, proto, thrift (already done), wasm, wat
- [ ] Markup/template tier (inject into embedded languages): html, markdown, jinja, liquid, erb,
      vue (already done), svelte (already done), astro
- [ ] Niche/legacy tier: awk, sed (trivial), make, cmake, meson, nix (already done), dhall,
      cue, nickel, kdl, ron

**Language implementation depth** (not a known limitation — a bug):
Most of the 98 language impls return empty for imports, complexity, docstrings, type extraction,
test detection etc. This is not "honest support" — it's a gap that must not be accepted. Each
language that silently returns empty is misleading users who expect analysis and get nothing.
- [ ] Audit: for each language, document which methods are genuinely unsupported by the grammar
      vs which are just unimplemented (the latter must be fixed, not accepted)
- [ ] Warning: when analysis returns empty because the language impl doesn't support it (not
      because the file has no symbols), surface a warning rather than silent empty output
- [ ] Prioritize: Python, JavaScript/TypeScript, Go, Java, C, C++, Ruby, Rust (already good)
      are the high-value targets — full implementations, not boilerplate
- [ ] Groovy: tags.scm references `class_definition`/`function_definition` but the tree-sitter
      grammar produces different node kinds — extraction returns nothing (zero symbols, zero imports)
- [ ] Kotlin: tree-sitter-kotlin fails to parse `val` declarations inside function bodies — any
      function containing `val x = ...` silently breaks the entire file parse (no symbols extracted)
- [ ] Kotlin/Scala/Groovy: import queries produce no results — import.scm patterns may not match
      actual grammar node structure (needs AST inspection to verify node kinds)
- [ ] Elixir: tags.scm doesn't match `def name do ... end` (no-args functions) — only matches
      `def name(args) do ... end` pattern. The `(arguments (call target: (identifier) @name))`
      pattern requires a call node, but no-args defs use `(arguments (identifier))`
- [ ] Haskell: type signatures and function definitions produce duplicate symbols (same name
      appears twice — once for `signature`, once for `function`). Should deduplicate.

**Comprehensive language fixtures** (long-term, verification via nix flakes):
Goal: for every language we support, a test suite that exercises the full extraction pipeline
and can be run in CI with real language toolchains provided by nix devShells/flake outputs.

- [ ] Design fixture schema: input source file → expected symbols, imports, calls, references
      (similar to existing syntax-rules fixtures but for extraction + scope resolution)
- [ ] Nix flake approach: each language's fixtures run in a devShell with the real compiler/runtime
      available — lets us verify against `rustc`, `tsc`, `python`, `go build` etc. for ground truth
- [ ] Fixture runner: language-agnostic test runner (like syntax-rules fixture runner) that loads
      `tests/fixtures/<lang>/locals/<case>/input.<ext>` + `expected.json` and diffs
- [ ] Seed fixtures for top 20 languages (high confidence, hand-verified)
- [ ] Automated fixture generation: use `normalize analyze` + LLM to bootstrap expected outputs,
      then human-verify before committing
- [ ] CI integration: `nix flake check` runs all language fixture suites in parallel

**Qualified/namespaced import resolution in the facts index:**
`find_callers(name)` is name-only — it will rename two unrelated `foo()` functions in different
modules simultaneously. Fix: store module-qualified caller/callee names in the index so lookups
resolve to a specific definition, not a name string.
- [ ] Store caller/callee with module qualification in facts index
- [ ] Post-filter in `find_callers`: verify callee resolves to definition file via import graph
- [ ] Update `edit rename` to use qualified lookup (eliminates false positives)

**Stretch goal: normalize as an LSP server (with optional proxy)**
- [ ] Implement core LSP methods backed by normalize's own reference resolution:
      `textDocument/references`, `textDocument/rename`, `textDocument/definition`,
      `textDocument/documentSymbol`, `workspace/symbol`
- [ ] LSP proxy mode: `normalize serve lsp --proxy 'rust-analyzer'` — forward requests to
      an arbitrary LSP command, use normalize as fallback or supplement
- [ ] Editor integration: VS Code extension, Neovim config — use normalize LSP for languages
      without a native server, proxy for languages that have one

### Lint / Analysis Architecture

See `docs/lint-architecture.md` for full design discussion.

**Architecture decision: Datalog for semantic queries**
- Datalog is the standard for code analysis (CodeQL, Semmle, codeQuest)
- Recursion essential for code queries (transitive deps, call graphs)
- Safe Datalog: guaranteed termination, right level of expressiveness

**Implementation plan:**
- [ ] All rules (builtin + user) compile to dylibs via Ascent + `abi_stable`
- [ ] Same infrastructure for both - builtins ship pre-compiled, users compile theirs
- [ ] Same syntax for both (rules can graduate from user to builtin)
- See "Facts & Rules Architecture" section below for full plan

**Rule tiers:**
1. `syntax-rules` (exists): AST patterns, no facts needed
2. `facts-rules` (new): Datalog over extracted facts (symbols, imports, calls)
3. `normalize-lint` (new): escape hatch for complex imperative logic

**Differentiation from CodeQL:**
- CodeQL: deep analysis (types, data flow, taint), ~12 languages
- normalize: structural/architectural analysis, ~98 languages
- Focus areas: circular deps, unused exports, module boundaries, import graph metrics

**Backlog - Deep Analysis (CodeQL-style):**
- [ ] Type extraction for top languages (TS, Python, Rust, Go)
- [ ] Data flow analysis
- [ ] Taint tracking
- Note: significant per-language effort, but tractable with LLM assistance

**Architectural analysis next iteration:**
- [ ] Boundary violation rules (configurable: "services/ cannot import cli/")
- [ ] Re-export tracing (follow `pub use` to resolve more imports)

Rules (custom enforcement, future):
- [ ] Module boundary rules ("services/ cannot import cli/")
- [ ] Threshold rules ("fan-out > 20 is error")
- [ ] Dependency path queries ("what's between A and B?")

**Rule tags system:**
- [ ] Deterministic tag color hashing in `--pretty` output (curated palette, red/yellow reserved for severity)

**Facts & Rules Architecture:**

- [ ] `normalize rules compile <rules.dl>` command to build custom packs (sandboxed codegen)
- [ ] Self-install builtin dylib: `normalize rules run --engine fact` should auto-install compiled builtins to `~/.local/share/normalize/rules/` on first run (or at build/install time). Currently requires manual copy.

### Language Capability Traits

See `docs/language-capability-traits.md` for full design.

The monolithic `Language` trait couples two growth axes: adding a language requires implementing all methods, adding a feature requires sweeping all 98 impls. Split into optional capability traits, following the `LocalDeps` precedent.

Trigger: split a capability when >50% of languages would return stubs. `has_symbols()` is the existing smell.

- [ ] `LanguageEmbedded` — extract `embedded_content()`, already past sparsity threshold (only Vue, HTML, ~3 others)
- [ ] Add `as_symbols()`, `as_imports()`, `as_complexity()`, `as_edit()` query methods to `Language` with `None` defaults (Option B from design doc — incremental, no flag-day)
- [ ] Migrate call sites to use capability queries where "not supported" differs from "empty"
- [ ] Remove `has_symbols()` once capability queries cover all its uses

### normalize-typegen

**Input Parsers:**
- [ ] Protobuf parser - read .proto files to IR
- [ ] GraphQL schema parser - read GraphQL SDL to IR

**Output Backends:**
- [ ] JSON Schema output - emit IR back to JSON Schema (for validation/documentation)
- [ ] GraphQL SDL output - emit IR as GraphQL types
- [ ] Protobuf output - emit IR as .proto definitions

**CLI Enhancements:**
- [ ] Multiple output files (`--split` to emit one file per type)
- [ ] Dry-run mode (`--dry-run` to preview without writing)

**IR Improvements:**
- [ ] Validation: ensure IR is well-formed before generating (no circular refs, valid names)
- [ ] Nullable vs Optional distinction (some languages care)
- [ ] Default values support in Field
- [ ] Constraints (min/max, pattern, format) for validation libraries

### normalize-surface-syntax

**Readers:**
- TypeScript reader: missing classes/interfaces/type annotations, spread/destructuring, template literals, async/await
- Lua reader: missing metatables/metamethods, string methods (`:method()` syntax)
- [ ] JavaScript reader (or reuse TypeScript reader with flag?)

**Writers:**
- Lua writer: verify idiomatic output (use `and`/`or` vs `&&`/`||`), string escaping edge cases
- TypeScript writer: type annotations, semicolon placement verification, template literal output
- [ ] JavaScript writer (or reuse TypeScript writer?)

**Testing:**
- [ ] Edge case tests: nested expressions, complex control flow, Unicode strings

**IR Improvements:**
- [ ] Comments preservation (for documentation translation)
- [ ] Source locations (for error messages, debugging)
- [ ] Import/export statements
- [ ] Class definitions, method definitions
- [ ] Type annotations (optional, for typed languages)
- [ ] Pattern matching / destructuring

### Complexity Hotspots (reduced - max now 58)
- [ ] `crates/normalize/src/commands/analyze/query.rs:cmd_query` (58)
- [ ] `crates/normalize/src/commands/daemon.rs:cmd_daemon` (54)
- [ ] `crates/normalize-syntax-rules/src/runner.rs:evaluate_predicates` (53)
- [ ] `crates/normalize/src/commands/analyze/mod.rs:run` (51)
- [ ] `crates/normalize/src/commands/tools/lint.rs:cmd_lint_run` (48)
- [ ] `crates/normalize/src/tree.rs:collect_highlight_spans` (46)

### Package Index Backlog (simplest → complex)

**6. Crates.io db-dump** (hard - large dataset)
- [ ] Download and parse db-dump.tar.gz (~800MB compressed)
- [ ] ~150k crates, refreshed daily
- [ ] Would need tar + csv parsing

**7. npm registry** (hard - massive scale)
- [ ] Evaluate registry replicate API feasibility
- [ ] ~3M packages, would need incremental CouchDB replication
- [ ] Consider: is this worth implementing? Search/fetch covers most use cases

**8. PyPI bulk** (hard - no native API)
- [ ] Research alternatives to simple index scraping
- [ ] Consider BigQuery dataset (google-cloud-pypi) or warehouse.pypi.org
- [ ] Consider: is this worth implementing? fetch() covers most use cases

### Code Quality
- Unnecessary aliases: `let x = Foo; x.bar()` → `Foo.bar()`. Lint for pointless intermediate bindings.
- PR/diff analysis: `normalize analyze --pr` or `--diff` for changed code focus (needs broader analysis workflow design)
- Deduplicate SQL queries in normalize: many ad-hoc queries could use shared prepared statements or query builders (needs design: queries use different execution contexts - Connection vs Transaction)
- Detect reinvented wheels: hand-rolled JSON/escaping when serde exists, manual string building for structured formats, reimplemented stdlib. Heuristics unclear. Full codebase scan impractical. Maybe: (1) trigger on new code matching suspicious patterns, (2) index function signatures and flag known anti-patterns, (3) check unused crate features vs hand-rolled equivalents. Research problem.
- ~~**Structural fragment frequency analysis**~~: Done — `normalize analyze fragments`. Supports `--scope all|functions|blocks`, `--min-nodes N`, `--similarity` for fuzzy matching, `--skeleton`, `--entry` for symbol glob filtering. `--inline-depth` scaffolded but not yet wired (requires async index access in sync context).
- ~~**CLI entrypoint duplication analysis**~~: Partially done — `normalize analyze fragments --scope functions --entry 'pattern'` handles the filtering. Full callee inlining (`--inline-depth`) requires async index access, deferred.
- Remaining duplicate/clone detection improvements:
  - Per-subcommand excludes in config: `[analyze.similar-blocks] exclude = [...]` so language-file exclusion doesn't affect `analyze rules`, `analyze complexity`, etc. (currently the global `[analyze] exclude` is too coarse)
  - "Parallel impl directory" heuristic: if >N pairs originate from the same directory pair, fold them into a single suppressed note (e.g., "48 pairs suppressed within normalize-languages/ — likely parallel Language trait implementations")
  - **`duplicate-blocks` should elide literals by default** (opt-out with `--no-elide-literals`): structurally-identical blocks that differ only in string/number literals are real duplication. Verified false negative: the three score-breakdown rows in `health.rs` (`format_pretty`) are identical structure with different field names/labels — caught by `--elide-literals` but missed by default. `similar-blocks` has no `--elide-literals` at all (add it).
  - `similar-blocks` / `similar-functions`: cross-file same-containing-function suppression covers same-method-name in different files; doesn't cover same-body-pattern across different method names (the Language impl case)
  - Consider min-lines bump for `similar-blocks` (currently 10) — the 19-line Symbol constructor is below many useful thresholds; maybe 15-20 default would further cut noise without missing real clones
- Phase 3b builtin rules: more builtin rules, sharing improvements (see `docs/design/builtin-rules.md`)
  - Semantic rules system: for rules needing cross-file analysis (import cycles, unused exports, type mismatches). Current syntax-based rules are single-file AST queries; semantic rules need index-backed analysis. Separate infrastructure, triggered differently (post-index vs per-file).

### Workflow Engine
- JSON Schema for complex action parameters (currently string-only)
- Workflow chaining: automatically trigger next workflow based on outcome (e.g., Investigation → Fix → Review)

### Script System
- TOML workflow format: structured definition (steps, actions) - **deferred until use cases are clearer**
  - Builtin `workflow` runner script interprets TOML files
  - Users can also write pure Lua scripts directly
- Lua test framework: test discovery for `.normalize/tests/` (test + test.property modules done)
  - Command naming: must clearly indicate "normalize Lua scripts" not general testing (avoid `@test`, `@spec`, `@check`)
  - Alternative: no special command, just run test files directly via `normalize <file>`
- Type system uses beyond validation
  - Done: `T.describe(schema)` for introspection, `type.generate` for property testing
  - Future: extract descriptions from comments (LuaDoc-style) instead of `description` field
- Format libraries (Lua): json, yaml, toml, kdl - **very low priority, defer until concrete use case**
  - Pure Lua implementations preferred (simple, no deps)
  - Key ordering: sort alphabetically by default, `__keyorder` metatable field for explicit order

### Tooling
- Read .git directly instead of spawning git commands where possible
  - Default branch detection, diff file listing, etc.
  - Trade-off: faster but more fragile (worktrees, packed refs, submodules)
- Documentation freshness: tooling to keep docs in sync with code
  - For normalize itself: keep docs/cli/*.md in sync with CLI behavior (lint? generate from --help?)
  - For user projects: detect stale docs in fresh projects (full normalize assistance) and legacy codebases (missing/outdated docs)
  - Consider boy scout rule: when touching code, improve nearby docs
- `normalize fetch`: web content retrieval for LLM context (needs design: chunking, streaming, headless browser?)
- Semantic editing next steps:
  - Structural search-replace: `--pattern 'fn $name($args) -> $ret { ... }'` AST-level, not regex
  - Integration with shadow git: checkpoint before large refactors, rollback on failure
  - **Local rename (`edit rename path/func/local new_name`)**: scoped rename within a block.
    No index needed. Two tiers:
    - Conservative: `replace_all_words` within the container's byte range, stop at any nested
      binding with the same name (avoids worst-case shadowing corruption, misses outer refs past inner shadow)
    - Correct: tree-sitter scope walk — find the declaration node, then walk identifier nodes
      that resolve to the same binding. Language-specific scope rules required (Rust, JS, Python differ).
    SkeletonExtractor doesn't surface locals; needs a dedicated local-variable locator.
- Cross-file refactors: `normalize move src/foo.rs/my_func src/bar.rs`
  - Move functions/types between files with import updates
  - Handles visibility changes (pub when crossing module boundaries)
  - Updates callers to use new path
- Structured config crate (`normalize-config`): trait-based view/edit for known config formats (TOML, JSON, YAML, INI). Unified interface across formats. (xkcd 927 risk acknowledged)
  - Examples: .editorconfig, prettierrc, prettierignore, oxlintrc.json[c], oxfmtrc.json[c], eslint.config.js, pom.xml
  - Open: do build scripts belong here? (conan, bazel, package.json, cmake) - maybe separate `normalize-build`
  - Open: linter vs formatter vs typechecker config - same trait or specialized?
  - Open: reconsider normalize config format choice (TOML vs YAML, JSON, KDL) - rationalize decision

### Workspace/Context Management
- Persistent workspace concept (like Notion): files, tool results, context stored permanently
- Cross-session continuity without re-reading everything
- Investigate memory-mapped context, incremental updates

### Package Management
- `normalize package install/uninstall`: proxy to ecosystem tools (cargo add, npm install, etc.)
  - Very low priority - needs concrete use case showing value beyond direct tool usage
  - Possible value-adds: install across all ecosystems, auto-audit after install, config-driven installs

### Agent Future

Core agency features complete (shadow editing, validation, risk gates, retry, auto-commit).

**Remaining:**
- [ ] Test selection: run only tests affected by changes (use call graph). Related: `analyze test-gaps` (see `docs/design/test-gaps.md`) shares the test-context classification
- [ ] Task decomposition: break large tasks into validated subtasks
- [ ] Cross-file refactoring: rename symbol across codebase
- [ ] Partial success: apply working edits, report failures
- [ ] Human-in-the-loop escalation: ask user when stuck

**RLM-inspired** (see `docs/research/recursive-language-models.md`):
- [ ] Recursive investigation: agent self-invokes on subsets (e.g., `view --types-only` → pick symbols → `view symbol` → recurse if large)
- [ ] Decomposition prompting: system prompt guides "search before answering" strategy
- [ ] Chunked viewing: `view path --chunk N` or `view path --around "pattern"` for large files
- [ ] REPL-style persistence: extend ephemeral context beyond 1 turn for iterative refinement
- [ ] Depth/cost limits: cap recursion depth, token budgets per investigation

### Agent / MCP
- Gemini Flash 3 prompt sensitivity: certain phrases ("shell", "execute", nested `[--opts]`) trigger 500 errors. Investigate if prompt can be further simplified to avoid safety filters entirely. See `docs/design/agent.md` for current workarounds.
- `normalize @agent` MCP support as second-class citizen
  - Our own tools take priority, MCP as fallback/extension mechanism
  - Need to design how MCP servers are discovered/configured
- Context view management: extend/edit/remove code views already in agent context
  - Agents should be able to request "add more context around this symbol" or "remove this view"
  - Incremental context refinement vs full re-fetch

### Session Analysis Backlog

**Bug: `Turn::token_usage` only captures the last API call per turn.** In `claude_code.rs`, `last_request_id` is overwritten on each assistant entry — so multi-round turns (user → tool call → tool result → final answer) only account for the final API call. Fix: accumulate all `requestId`s seen within a turn (`turn_request_ids: Vec<String>`) and sum their `request_tokens` on flush.

**Composable message filters:**
- `--has-tool <name>` — messages in turns that used a specific tool
- `--min-chars <N>` / `--max-chars <N>` — filter by message length (not just truncation)
- `--errors-only` — turns with tool errors
- `--turn-range <start>-<end>` — positional filtering within sessions
- `--exclude-interrupted` — skip `[Request interrupted by user]` noise

**Analysis features:**
1. **Cross-repo comparison**: group sessions by repository, compare metrics: tool usage, error rates, parallelization, costs. `--by-repo` flag to stats command.
2. **Ngram analysis**: extract common word sequences from assistant messages (bigrams/trigrams/4-grams). Find common error messages, repeated explanations, boilerplate responses.
3. **Parallelization hints**: beyond counting, show specific turns with sequential independent calls. Example: `Turn 12: ⚠️ Could parallelize: Read(foo.rs) → Read(bar.rs) → Read(baz.rs)`
4. **File edit heatmap**: which files churned most? Files read but never edited: potential test gaps. Files edited multiple times: fragile design or iterative refinement?
5. **Cost breakdown**: model-specific pricing, cache savings display, per-turn cost tracking.

**Other session analysis:**
- Web syntax highlighting: share tree-sitter grammars between native and web SPAs
  - Option A: embed tree-sitter WASM runtime, load .so grammars
  - Option B: `/api/highlight` endpoint, server-side highlighting
- Antigravity conversations: `~/.gemini/antigravity/conversations/*.pb` (protobuf - needs schema, files appear encrypted)
- Antigravity brain artifacts: `~/.gemini/antigravity/brain/*/` (task/plan/walkthrough metadata)
- Additional agent formats (need to find log locations/formats):
  - Windsurf (Codeium)
  - Cursor
  - Cline
  - Roo Code
  - Gemini Code Assist (VS Code extension)
  - GitHub Copilot (VS Code)
- Better `--compact` format: key:value pairs, no tables, all info preserved
- Better `--pretty` format: bar charts for tools, progress bar for success rate
- `normalize sessions mark <id>`: mark as reviewed (store in `.normalize/sessions-reviewed`)
- Agent habit analysis: study session logs to identify builtin vs learned behaviors
  - Example: "git status before commit" - is this hardcoded or from CLAUDE.md guidance?
  - Test methodology: fresh/empty repo without project instructions
  - Cross-agent comparison: Claude Code, Gemini CLI, OpenAI Codex, etc.
  - Goal: understand what behaviors to encode in normalize agent (model-agnostic reliability)
  - Maybe: automated agent testing harness (run same tasks across assistants)

### Friction Signals (see `docs/research/agent-adaptation.md`)
How do we know when tools aren't working? Implicit signals from agent behavior:
- Correction patterns: "You're right", "Should have" after tool calls
- Long tool chains: 5+ calls without acting
- Tool avoidance: grep instead of normalize, spawning Explore agents
- Follow-up patterns: `--types-only` → immediately view symbol
- Repeated queries: same file viewed multiple times

### Global rules exclude in config

`normalize rules run` has no global path exclude — every error rule needs `.claude/**` added
to its `allow` list separately to prevent false positives from agent worktrees under `.claude/`.
Added `.claude/**` to `rust/tuple-return`, `no-grammar-loader-new`, `rust/chained-if-let`,
`rust/numeric-type-annotation` as a workaround. Need a `[rules] exclude = [...]` config key
that applies before per-rule allow lists. Alternatively: pre-commit hook should pass
`--root crates/` or an `--exclude .claude/` flag.

### CI/Infrastructure
- [ ] Wire `normalize analyze duplicate-blocks --exclude '**/*.json' --exclude '**/*.lock'` into CI

### Distribution
- Wrapper packages for ecosystems: npm, PyPI, Homebrew, etc.
  - Auto-generate and publish in sync with GitHub releases
  - Single binary + thin wrapper scripts per ecosystem
- Direct download: platform-detected link to latest GitHub release binary (avoid cargo install overhead)

### Vision (Aspirational)
- **Friction Minimization Loop**: normalize should make it easier to reduce friction, which accelerates development, which makes it easier to improve normalize. Workflows documented → failure modes identified → encoded as tooling → friction reduced → faster iteration. The goal is tooling that catches problems automatically (high reliability) not documentation that hopes someone reads it (low reliability).
- Verification Loops: domain-specific validation (compiler, linter, tests) before accepting output
- Synthesis: decompose complex tasks into solvable subproblems (`normalize synthesize`)
- Plugin Architecture: extensible view providers, synthesis strategies, code generators

## Known Issues

### normalize-languages: ast-grep test broken
The `ast_grep::tests::test_pattern_matching` test fails to compile due to API mismatch:
- `DynLang.parse()` method not found
- `ast_grep_core::tree_sitter::LanguageExt` trait may need explicit import or implementation
- Pre-existing issue, not caused by feature flag changes

## Long-Term Goals

### Incremental-first architecture
The current architecture is batch-oriented: commands scan the whole workspace, produce a report, and exit. This works for CLI but is wrong for LSP and other interactive consumers. The goal is to make incrementality a first-class concern throughout the stack.

**Where batch hurts today:**
- LSP diagnostics re-run all rule engines on every save (syntax rules re-parse every file, fact rules rebuild the full index)
- `FileIndex` is rebuilt from scratch — no way to update a single file's symbols/imports/calls
- Syntax rules load and compile all tree-sitter queries on every invocation

**Target architecture:**
- **FileIndex**: `update_file(path, content)` — re-index one file, update SQLite incrementally (delete old rows, insert new). Dependency graph tracks which files' diagnostics to invalidate.
- **Syntax rules**: per-file evaluation. On save, re-run rules only on the saved file. Cache compiled queries across invocations (already cached per-process via `GrammarLoader`, but lost between LSP requests).
- **Fact rules**: incremental Datalog. When facts for one file change, re-derive only affected conclusions. This is hard — may need semi-naive evaluation with change tracking, or accept batch for fact rules and optimize syntax rules first.
- **Watch mode**: `normalize watch` that keeps the index live and re-runs checks on file changes (inotify/fsevents). The LSP server is one consumer; a TUI dashboard could be another.

**Incremental steps (not all-or-nothing):**
1. `FileIndex::update_file()` — single-file re-index without full rebuild
2. Per-file syntax rule evaluation in LSP (run rules only on saved file)
3. Persistent `GrammarLoader` in LSP (don't re-create `SkeletonExtractor` per request)
4. File-level dependency tracking for diagnostic invalidation
5. Incremental fact rule evaluation (long-term, research needed)

## Deferred

- `normalize jq` multi-format support (YAML/CBOR/TOML/XML via `jaq-all` with `formats` feature): currently using `jaq-core/std/json` directly to avoid `jaq-fmts` bloat. Low priority — vanilla jq is JSON-only anyway.

## Embedded CLI drop-in integrations (see docs/cli-dropin-integrations.md)

All three integrations complete: `jq`, `rg`, `ast-grep`.

Completed:
- `normalize ast-grep --rewrite` / `--interactive` (ast-grep-config::Fixer + crossterm)
- `normalize ast-grep scan` (project config, rule discovery, CombinedScan)
- `normalize ast-grep test` (rule verification, snapshots, interactive reporter)

Future work:
- `normalize rg` PCRE2 support (pcre2 feature not enabled)



- VS Code extension: test and publish to marketplace (after first CLI release)
- Remaining docs: prior-art.md, hybrid-loops.md
- Memory system: `docs/design/memory.md` — SQLite-backed `store/recall/forget`. Deferred until concrete use case.

## Implementation Notes

### Self-update (`normalize update`)
- Now in commands/update.rs
- GITHUB_REPO constant → "rhi-zone/normalize"
- Custom SHA256 implementation (Sha256 struct)
- Expects GitHub release with SHA256SUMS.txt

## When Ready

### First Release
```bash
git tag v0.1.0
git push --tags
```
- Verify cross-platform builds in GitHub Actions
- Test `normalize update` against real release
- view: directory output shows dir name as first line (tree style) - intentional?

## Syntax Ruleset Breadth

After batch-fixing the current info violations, audit and expand rule coverage:
- **What we have**: ~20 builtin rules, mostly Rust-focused. Good Rust coverage; thin everywhere else.
- **Next**: flesh out rules for JS/TS, Python, Go, Ruby — languages with large userbases and well-known anti-patterns.
- **Trigger for fix infrastructure**: once enough rules have structural auto-fixes that need correct indentation, build the corpus-based indentation model (see `docs/prior-art.md` § "Corpus-based indentation model"). Don't build it speculatively.
- **Rule ideas by language**:
  - JS/TS: `var` usage, `== null` vs `=== null`, `typeof` checks, async/await anti-patterns
  - Python: mutable default args, bare `except`, `assert` in non-test code
  - Go: error ignored (`_ = err`), `fmt.Println` in non-main, unnecessary `return` at end
  - Ruby: `rescue Exception`, `puts` in non-script, string interpolation over concatenation
  - Cross-language: hardcoded credentials (already have), magic numbers, commented-out code blocks

## Fix System: Structural Rewrites (post text-replacement)
- **Sexpr-based fix expressions**: The current `fix = "template $capture"` is text replacement. For structural transforms (indentation-aware, composable), consider expressing fixes as output tree patterns rather than strings. eglint (~/git/eglint) does this for TypeScript — useful prior art for the approach even though it's TS-compiler-specific and doesn't port directly.
- **Fix fixture tests**: Infrastructure added (`fix.<ext>` + `fix.expected.<ext>` in fixture dirs; temp dir created inside fixture dir for Cargo.toml walk-up). `rust/chained-if-let` covered. Adversarial cases (nested violations, near-EOF, overlapping) not yet added. Deletion rules (`breakpoint`, `binding-pry`, `console-log`) had `fix = ""` removed — auto-delete is too aggressive for statements that may be intentional.
- **eglint findings**: ~/git/eglint — reference-based AST formatting (not tree-sitter). Core insight: IndentNode/NewlineNode carry `deltaIndent` so indentation is computed at stringify time, not baked into captured text. InterchangeableNode/ForkNode for multiple formatting options avoids explicit conflict resolution. Would require language-specific pretty-printers to adopt — non-trivial.
