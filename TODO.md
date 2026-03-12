# Normalize Roadmap

Last triaged: 2026-03-12

See `CHANGELOG.md` for completed work. See `docs/` for design docs.

## Goal

Production-grade refactoring across all ~98 languages. Goal: rename, find-references,
extract, inline, move — correct, without LSPs, without false positives.

---

## P0 — Blocking / Broken / Incoherent

### server-less UX issues — ~~all fixed~~ (server-less commit 9c294b2)

1. ~~**`name` attribute ignored for nested services**~~: Fixed — `#[cli(name = "...")]` now works on individual methods (leaf and mount). `get_cli_name()` helper added.
2. ~~**No error for helper methods in `#[cli]` block**~~: Fixed — added `#[cli(helper)]` as a self-documenting alias for `#[cli(skip)]`. Module docs updated.
3. ~~**`display_with` across impl blocks is non-obvious**~~: Fixed — module docs now explicitly document that `display_with` functions can live in any impl block on the same type.

### ~~Session analysis bug~~ (already fixed)

~~**Bug: `Turn::token_usage` only captures the last API call per turn.**~~ Already fixed in claude_code.rs — `turn_request_ids: Vec<String>` accumulates all request IDs and `sum_turn_tokens` sums them on flush.

### LSP diagnostics improvements

- [x] Per-file syntax rules (only re-run on the saved file, not the whole workspace)
- [x] Incremental index update on save via `FileIndex::update_file()`
- [x] Two-tier diagnostics: immediate syntax, debounced (1500ms) fact rules
- [x] Daemon calls `incremental_call_graph_refresh()` after detecting changes
- [x] Persistent `SkeletonExtractor` in LSP backend (avoids recreating per request)
- [x] Compiled query caching in `GrammarLoader` (tags, imports, calls, complexity)
- [x] Configurable debounce interval (`[serve] fact_debounce_ms`, default 1500)
- Incremental Datalog for fact rules — **blocked on ascent-interpreter upstream** (ask user for status periodically)
  Agreed roadmap (with ascent-interpreter maintainer):
  1. String interning (makes most values u32)
  2. Flat tuple storage (relations store typed arrays, arity-specialized `[u32; N]`)
  3. **Incremental evaluation** — file-scoped retraction, strata invalidation, persisted engine.
     This is the highest-value step for LSP: process 50 changed facts instead of re-evaluating 500k.
     After interning, diffing facts is just comparing `[u32; N]` arrays → cheap file-scoped retraction.
  4. Bytecode for expressions (measure after step 2 — joins may dominate, not expression eval)
  5. Arity-specialized eval routines (generic over `[u32; N]`, stamped out via macro)
  6. Cranelift JIT (feature-gated; defer decision until after step 4)
- File-level dependency tracking (import graph edges to scope fact re-evaluation)
- `normalize watch` CLI (expose daemon file-watching with TUI output)
- [x] Progress reporting for `structure rebuild` (indicatif bars for file scan, symbol parsing, index storage)
- [x] Progress reporting for `analyze duplicates`, `analyze architecture`, `analyze duplicate-types` (indicatif bars for file processing, spinners for architecture phases)

---

## P1 — Short-term Improvements (coherence / usability)

### Main Crate Responsibility Boundaries

Size isn't the concern — multiple responsibilities in one crate is. Extract when reusable
domain logic is trapped in the CLI crate and a real second consumer exists (LSP, external
tool, another command). Don't extract for line count alone.

**Candidates (extract when a second consumer appears):**
- `serve/` (LSP + HTTP + MCP) → `normalize-serve`
- `src/analyze/` (pure computation) → `normalize-architecture`
- `commands/sessions/` — circular dep risk, needs care

### Analyze Command Consolidation — remaining work

**Current: 42 commands** (was 44; `analyze parse` and `analyze query` deleted 2026-03-12 — duplicates of `syntax ast`/`syntax query`). All Phase 2/3 merges that were feasible have been completed.

**Phase 3 rank infrastructure (in progress, 2026-03-12):**
- `RankEntry` trait + `Column`/`Align` + `format_ranked_table()` in `normalize-analyze::ranked` — shared tabular rendering for all rank-pattern commands
- Migrated 13 commands: files, imports, ownership, docs, ceremony, surface, depth-map, layering, test-ratio, budget, density, coupling, uniqueness
- Not migrated (conditional columns): hotspots (has_complexity flag changes column set)
- Not migrated (different structure): complexity, length (use `FileReport<T>` with `FullStats`), test-gaps (not tabular), coupling-clusters (prose), architecture, call-complexity
- `DiffableRankEntry` trait + `compute_ranked_diff()` + `format_delta()` in `normalize-analyze::ranked` — generic `--diff <ref>` support for rank commands
- Added `--diff` to all 12 rank commands: test-ratio, density, uniqueness, files, imports, ownership, ceremony, surface, depth-map, layering, budget, coupling
- Async commands (imports, surface, depth-map, layering) use `block_in_place` + `ensure_ready` in worktree for baseline
- `--trend` already generic via `analyze_scalar_trend` (4 existing trend commands use it)

**Future (low priority):** `security` → SARIF rules engine (wraps bandit; could be `normalize rules run --engine sarif` with bandit configured). `docs`/`security` → rules migration (~-3 commands, see design doc).

**Design pressure:** ~42 commands is still too spread out. Phase 3 must continue. The goal is a surface small enough that a user can hold it in working memory — not just "fewer than 49".

### Language trait: remaining .scm migration

**Known locals.scm scope engine limitation:**
- Nested destructuring (e.g. `{ a: { b } }` in parameters) requires recursive queries which
  tree-sitter does not support. One level of object/array destructuring IS covered for JS/TS/TSX.
  Fixing deeper nesting would require engine-level recursion (walk into nested patterns).

### Language implementation depth

- [x] Audit (2026-03-12): 47/84 languages at 100% .scm coverage. Full gap list below.

**Feasible gaps (grammar supports it, .scm not written):**

High-value:
- [x] TSX imports.scm (reuse TypeScript logic)
- [x] ~~Svelte imports.scm~~ — grammar produces opaque import_statement nodes; extraction handled by Rust extract_imports() + embedded JS injection
- [x] ~~Vue imports.scm~~ — grammar doesn't parse JS content; extraction depends entirely on embedded JS injection
- [x] ~~GraphQL imports.scm~~ — genuinely no import syntax in grammar (federation directives are just regular directives)
- [x] ~~SQL imports.scm~~ — genuinely no import syntax modeled in grammar (IMPORT FOREIGN SCHEMA not in tree-sitter-sql)
- [x] Jinja2 calls.scm (function/method/filter/test/call-statement)
- [x] Thrift tags.scm (struct/union/exception/enum/service/function/typedef/const)
- [x] Dockerfile tags.scm (FROM...AS stages, ARG, ENV)

80% languages (types.scm assessed — only Typst feasible):
- [x] Typst types.scm (parameter type annotations via `tagged` nodes in `let` bindings)
- [x] ~~SCSS types.scm~~ — no type system; grammar has no type-like nodes
- [x] ~~Perl types.scm~~ — no static type annotations in grammar (Perl is dynamically typed; type constraints via Moose/Type::Tiny are runtime, not in CST)
- [x] ~~Prolog types.scm~~ — no type system in grammar (Prolog is untyped; type annotations via library predicates are just regular terms)
- [x] ~~AWK types.scm~~ — no type system; all values are strings/numbers contextually
- [x] ~~Fish types.scm~~ — shell language, no type annotations in grammar
- [x] ~~Zsh types.scm~~ — shell language, no type annotations in grammar
- [x] ~~Vim types.scm~~ — VimScript has no type annotations in grammar
- [x] ~~Jq types.scm~~ — no type system; jq operates on JSON values dynamically
- [x] ~~Meson types.scm~~ — build system DSL, no type annotations in grammar
- [x] ~~CMake types.scm~~ — build system scripting, no type annotations in grammar

Config/markup:
- [x] Nginx calls.scm (simple_directive and block_directive name captures)
- [x] Caddy tags.scm (site blocks, snippets, named matchers, handle/route directives)

**Genuinely unsupported (correct as None/empty):**
- Bash types (no type system)
- JSON/YAML/TOML/XML/Markdown imports/calls/complexity (data formats)
- HTML/CSS calls/complexity/types (markup/style)

### Comprehensive language fixtures (long-term, nix flake verification)

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

### ~~Qualified/namespaced import resolution in the facts index~~ (done 2026-03-12)

`calls` table now has `callee_resolved_file TEXT` column (schema v6). After import resolution,
`resolve_all_calls()` populates it by joining calls with imports and same-file symbol defs.
`find_callers()` uses `callee_resolved_file` for precise disambiguation; falls back to
import-based matching when NULL (external/unresolved modules).
- [x] Store caller/callee with module qualification in facts index (`callee_resolved_file` column)
- [x] Resolve calls after imports in both full and incremental refresh paths
- [x] `find_callers` uses resolved file — `edit rename`, `call-graph`, LSP references all benefit

### Remaining work (short items)

- Namespace-qualified lookups: `normalize view std::vector`, `normalize view com.example.Foo`
  - Requires language-specific namespace semantics - low priority
- Shadow worktree: true shadow-first mode (edit in shadow, then apply)
  - Current: --shadow flag works, but not default for all edits
  - Zero user interruption (user can edit while agent tests in background)

### Configuration system

Sections: `[daemon]`, `[index]`, `[aliases]`, `[view]`, `[analyze]`, `[grep]`, `[pretty]`, `[serve]`

Adding a new section (3 places):
1. Define `XxxConfig` struct with `#[derive(Merge)]` + `XxxArgs` with `#[derive(Args)]` in command module
2. Add field to NormalizeConfig
3. Add `run(args, json)` function that loads config and merges

Candidates: `[workflow]` (directory, auto-run)

### Schema-Driven Config UI — remaining follow-ups

- `normalize rules validate` intentionally separate: rule-ID validation (checks against live registry) can't be expressed in JSON Schema. Not redundant with `config validate` — they check different things.
- Extract engine into `normalize-config-ui` crate for reuse / publication (stretch goal)
- Propose `#[config]` proc macro to server-less (stretch goal — superseded by `#[derive(Config)]`;
  filed nested struct support + merge semantics requests in server-less TODO.md 2026-03-10)

### ~~Complexity Hotspots~~ (resolved - max now 22)

All original hotspots resolved. Remaining max is `split_query_patterns` (22) in runner.rs.

- [x] `crates/normalize/src/commands/analyze/query.rs:cmd_query` (58→15) — already resolved
- [x] `crates/normalize/src/commands/daemon.rs:cmd_daemon` (54→1) — already resolved
- [x] `crates/normalize-syntax-rules/src/runner.rs:run_rules` (53→18)
- [x] `crates/normalize-syntax-rules/src/runner.rs:evaluate_predicates` (53→11)
- [x] `crates/normalize/src/commands/analyze/mod.rs:run` (51→5) — already resolved
- [x] `crates/normalize/src/commands/tools/lint.rs:cmd_lint_run` (48→15) — already resolved
- [x] `crates/normalize/src/tree.rs:collect_highlight_spans` (42→9)
- [x] `crates/normalize/src/tree.rs:capture_name_to_highlight_kind` (23→2)
- [x] `crates/normalize/src/tree.rs:render_highlighted` (23→8)
- [x] `crates/normalize/src/tree.rs:docstring_style_for_grammar` (21→5)

### CLI Internal Consolidation

**Top-level command level issues (low priority):**
- `context` could be `normalize view context [path]` but semantics differ slightly (content-only vs prepend). Low priority.
- `aliases` is a cross-cutting utility. Too small for top-level but has no clear parent. Low priority.

### `normalize init --setup` extensions

**Remaining:** Review default-enabled rule set. Several rules generate high noise; guided
setup is the cure. Also: `normalize init --setup` currently only covers rules — extend to
other project-level decisions as they emerge (e.g., exclude patterns, SUMMARY.md enforcement).

**Default-enabled inconsistencies (2026-03-13 audit):**
- Debug-print rules: Go (`fmt-print`) and Python (`print-debug`) enabled by default, but C/C++/Java/Kotlin/PHP/Rust/Swift/C# equivalents all disabled. Should be consistent.
- Correctness rules that should be enabled by default: `go/defer-in-loop` (bug: defer runs at function return), `go/sync-mutex-copied` (concurrency bug), `swift/force-unwrap` (crash — inconsistent with Rust unwrap being enabled), `python/raise-without-from` (lost traceback), `python/use-with` (resource leak), `ruby/method-missing` (footgun without `respond_to_missing?`).
- Potentially too aggressive defaults: `rust/chained-if-let` (error severity for style), `rust/numeric-type-annotation` (error for style), tuple-return rules (noisy on existing code).

**Wizard UX improvements:**
- Show rules with zero violations too (at least a summary count + pointer to `rules list`)
- Group by tag/category instead of flat violation-count sort
- Add batch operations: "enable all [correctness] rules", "disable all [style] rules"
- Add `recommended = true` frontmatter for genuine bug/correctness rules vs style opinions
- Show practical impact: "2 violations (quick fix)" vs "847 violations (major cleanup)"
- Standalone `normalize rules setup` command (don't require re-running `init`)

### SARIF engine actionable output

- `rules run --engine sarif` could show which SARIF tools had errors (not done)

### Duplicate/clone detection improvements

- [x] Per-subcommand excludes in config: `[analyze.duplicates] exclude = [...]` via `#[serde(flatten)]` HashMap on `AnalyzeConfig`. Wired into all analyze subcommands that accept `--exclude`: duplicates, complexity, length, docs, health, all, test-gaps, uniqueness, hotspots, files, size, coupling, coupling-clusters, ownership, fragments, skeleton-diff.
- [x] "Parallel impl directory" heuristic: if >=5 pairs originate from the same directory pair, fold them into a suppressed note (e.g., "388 pairs suppressed across 10 directory groups"). Applied to exact-functions, similar-functions, and similar-blocks when `!include_trait_impls`. Handles 2-location pairs and multi-location groups (up to 2 distinct directories).
- `similar-blocks` / `similar-functions`: cross-file same-containing-function suppression covers same-method-name in different files; doesn't cover same-body-pattern across different method names (the Language impl case)
- Consider min-lines bump for `similar-blocks` (currently 10) — the 19-line Symbol constructor is below many useful thresholds; maybe 15-20 default would further cut noise without missing real clones

### Syntax Ruleset Breadth

- **Trigger for fix infrastructure**: once enough rules have structural auto-fixes that need correct indentation, build the corpus-based indentation model (see `docs/prior-art.md` § "Corpus-based indentation model"). Don't build it speculatively.
- **tree-sitter-go note**: `block` → `statement_list` → statements. Queries must use `statement_list` as intermediate node; `(block (return_statement))` won't match.
- Phase 3b builtin rules: more builtin rules, sharing improvements (see `docs/design/builtin-rules.md`)
  - [x] Java rules (6): `system-print`, `empty-catch`, `print-stack-trace`, `magic-number`, `suppress-warnings`, `thread-sleep`
  - [x] C/C++ rules (4): `c/printf-debug`, `c/goto`, `c/magic-number`, `cpp/cout-debug`
  - [x] C# rules (6): `console-write`, `empty-catch`, `goto`, `magic-number`, `thread-sleep`, `suppress-warnings`
  - [x] Kotlin rules (5): `println-debug`, `empty-catch`, `magic-number`, `thread-sleep`, `suppress-warnings`
  - [x] Swift rules (5): `print-debug`, `empty-catch`, `magic-number`, `force-unwrap`, `thread-sleep`
  - [x] PHP rules (5): `debug-print`, `empty-catch`, `goto`, `magic-number`, `eval`
  - Semantic rules system: for rules needing cross-file analysis (import cycles, unused exports, type mismatches). Current syntax-based rules are single-file AST queries; semantic rules need index-backed analysis. Separate infrastructure, triggered differently (post-index vs per-file).

### ~~Rule tags system~~ (done)

- [x] Deterministic tag color hashing in `--pretty` output (FNV-1a hash, 10-color curated palette at OKLCH L≈0.65, red/yellow reserved for severity)

---

## P2 — Structural Improvements / Larger Refactors

### Rules Unification — remaining threads

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

### Incremental-first architecture

The current architecture is batch-oriented: commands scan the whole workspace, produce a report, and exit. This works for CLI but is wrong for LSP and other interactive consumers. The goal is to make incrementality a first-class concern throughout the stack.

**What's done:**
- [x] `FileIndex::update_file()` — single-file re-index without full rebuild
- [x] Per-file syntax rule evaluation in LSP (run rules only on saved file)
- [x] Two-tier LSP diagnostics: immediate syntax, debounced fact rules
- [x] Daemon calls `incremental_call_graph_refresh()` after detecting changes

**Remaining:**
- Syntax rules load and compile all tree-sitter queries on every invocation
- **Fact rules**: incremental Datalog. When facts for one file change, re-derive only affected conclusions. This is hard — may need semi-naive evaluation with change tracking.
- **Watch mode**: `normalize watch` that keeps the index live and re-runs checks on file changes (inotify/fsevents). The LSP server is one consumer; a TUI dashboard could be another.

**Next incremental steps:**
1. Persistent `GrammarLoader` in LSP (don't re-create `SkeletonExtractor` per request)
2. File-level dependency tracking for diagnostic invalidation
3. Incremental fact rule evaluation (long-term, research needed)

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

**Architectural analysis next iteration:**
- [ ] Boundary violation rules (configurable: "services/ cannot import cli/")
- [ ] Re-export tracing (follow `pub use` to resolve more imports)

Rules (custom enforcement, future):
- [ ] Module boundary rules ("services/ cannot import cli/")
- [ ] Threshold rules ("fan-out > 20 is error")
- [ ] Dependency path queries ("what's between A and B?")

**Facts & Rules Architecture:**
- [ ] `normalize rules compile <rules.dl>` command to build custom packs (sandboxed codegen)
- [ ] Self-install builtin dylib: `normalize rules run --engine fact` should auto-install compiled builtins to `~/.local/share/normalize/rules/` on first run (or at build/install time). Currently requires manual copy.

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

### Rust Redesign Candidates

- Rules engine: consider semgrep/ruff integration instead of custom
- Plugin system: Rust trait-based plugins or external tool orchestration

### Language Capability Traits — remaining

- [ ] Add `as_imports()`, `as_complexity()`, `as_edit()` capability queries — not ready yet. Sparsity check (2026-03-11, verified): `extract_imports` stub rate is ~1.4% (1 language: asm, with explicit comment). 72% have real impls; 7% use `.imports.scm` query files; 21% are config/data languages with no import concept (correct behavior). Far below 50% threshold — `as_imports()` trait is NOT warranted. Revisit after adding more languages.

### Tooling

- Read .git directly instead of spawning git commands where possible
  - Default branch detection, diff file listing, etc.
  - Trade-off: faster but more fragile (worktrees, packed refs, submodules)
- Documentation freshness: tooling to keep docs in sync with code
  - For normalize itself: keep docs/cli/*.md in sync with CLI behavior (lint? generate from --help?)
  - For user projects: detect stale docs in fresh projects (full normalize assistance) and legacy codebases (missing/outdated docs)
  - Consider boy scout rule: when touching code, improve nearby docs
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

### Code Quality

- Unnecessary aliases: `let x = Foo; x.bar()` → `Foo.bar()`. Lint for pointless intermediate bindings.
- PR/diff analysis: `normalize analyze --pr` or `--diff` for changed code focus (needs broader analysis workflow design)
- Deduplicate SQL queries in normalize: many ad-hoc queries could use shared prepared statements or query builders (needs design: queries use different execution contexts - Connection vs Transaction)
- Detect reinvented wheels: hand-rolled JSON/escaping when serde exists, manual string building for structured formats, reimplemented stdlib. Heuristics unclear. Full codebase scan impractical. Maybe: (1) trigger on new code matching suspicious patterns, (2) index function signatures and flag known anti-patterns, (3) check unused crate features vs hand-rolled equivalents. Research problem.
- Remaining duplicate/clone detection improvements:

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

### Distribution

- Wrapper packages for ecosystems: npm, PyPI, Homebrew, etc.
  - Auto-generate and publish in sync with GitHub releases
  - Single binary + thin wrapper scripts per ecosystem
- Direct download: platform-detected link to latest GitHub release binary (avoid cargo install overhead)

### Fix System: Structural Rewrites (post text-replacement)

- **Sexpr-based fix expressions**: The current `fix = "template $capture"` is text replacement. For structural transforms (indentation-aware, composable), consider expressing fixes as output tree patterns rather than strings. eglint (~/git/eglint) does this for TypeScript — useful prior art for the approach even though it's TS-compiler-specific and doesn't port directly.
- **Fix fixture tests**: Infrastructure added (`fix.<ext>` + `fix.expected.<ext>` in fixture dirs; temp dir created inside fixture dir for Cargo.toml walk-up). `rust/chained-if-let` covered. Adversarial cases (nested violations, near-EOF, overlapping) not yet added. Deletion rules (`breakpoint`, `binding-pry`, `console-log`) had `fix = ""` removed — auto-delete is too aggressive for statements that may be intentional.
- **eglint findings**: ~/git/eglint — reference-based AST formatting (not tree-sitter). Core insight: IndentNode/NewlineNode carry `deltaIndent` so indentation is computed at stringify time, not baked into captured text. InterchangeableNode/ForkNode for multiple formatting options avoids explicit conflict resolution. Would require language-specific pretty-printers to adopt — non-trivial.

---

## Aspirational — Research / Long-term Vision

### normalize as LSP server

- [ ] Implement core LSP methods backed by normalize's own reference resolution:
      `textDocument/references`, `textDocument/rename`, `textDocument/definition`,
      `textDocument/documentSymbol`, `workspace/symbol`
- [ ] LSP proxy mode: `normalize serve lsp --proxy 'rust-analyzer'` — forward requests to
      an arbitrary LSP command, use normalize as fallback or supplement
- [ ] Editor integration: VS Code extension, Neovim config — use normalize LSP for languages
      without a native server, proxy for languages that have one

### Deep Analysis (CodeQL-style)

- [ ] Type extraction for top languages (TS, Python, Rust, Go)
- [ ] Data flow analysis
- [ ] Taint tracking
- Note: significant per-language effort, but tractable with LLM assistance

### Trait-Based Extensibility

All trait-based crates follow the normalize-languages pattern for extensibility:
- Global registry with `register()` function for user implementations
- Built-ins initialized lazily via `init_builtin()` + `OnceLock`
- No feature gates (implementations are small, not worth the complexity)

Pattern: traits are the extensibility mechanism. Users implement traits in their own code, register at runtime. normalize CLI can add Lua bindings at application layer for scripting.

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

**Composable message filters:**
- `--has-tool <name>` — messages in turns that used a specific tool
- `--min-chars <N>` / `--max-chars <N>` — filter by message length (not just truncation)
- `--errors-only` — turns with tool errors
- `--turn-range <start>-<end>` — positional filtering within sessions
- `--exclude-interrupted` — skip `[Request interrupted by user]` noise

**Analysis features:**
1. **Cross-repo comparison**: group sessions by repository, compare metrics: tool usage, error rates, parallelization, costs. `--by-repo` flag to stats command.
2. **Ngram analysis**: extract common word sequences from assistant messages (bigrams/trigrams/4-grams). Find common error messages, repeated explanations, boilerplate responses.
3. **Parallelization hints**: beyond counting, show specific turns with sequential independent calls. Example: `Turn 12: Could parallelize: Read(foo.rs) → Read(bar.rs) → Read(baz.rs)`
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

### Workspace/Context Management

- Persistent workspace concept (like Notion): files, tool results, context stored permanently
- Cross-session continuity without re-reading everything
- Investigate memory-mapped context, incremental updates

### Package Management

- `normalize package install/uninstall`: proxy to ecosystem tools (cargo add, npm install, etc.)
  - Very low priority - needs concrete use case showing value beyond direct tool usage
  - Possible value-adds: install across all ecosystems, auto-audit after install, config-driven installs

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

### Vision

- **Friction Minimization Loop**: normalize should make it easier to reduce friction, which accelerates development, which makes it easier to improve normalize. Workflows documented → failure modes identified → encoded as tooling → friction reduced → faster iteration. The goal is tooling that catches problems automatically (high reliability) not documentation that hopes someone reads it (low reliability).
- Verification Loops: domain-specific validation (compiler, linter, tests) before accepting output
- Synthesis: decompose complex tasks into solvable subproblems (`normalize synthesize`)
- Plugin Architecture: extensible view providers, synthesis strategies, code generators

### First Release

```bash
git tag v0.1.0
git push --tags
```
- Verify cross-platform builds in GitHub Actions
- Test `normalize update` against real release
- VS Code extension: test and publish to marketplace (after first CLI release)

---

## Deferred

- `normalize jq` multi-format support (YAML/CBOR/TOML/XML via `jaq-all` with `formats` feature): currently using `jaq-core/std/json` directly to avoid `jaq-fmts` bloat. Low priority — vanilla jq is JSON-only anyway.
- `normalize rg` PCRE2 support (pcre2 feature not enabled)
- `normalize fetch`: web content retrieval for LLM context (needs design: chunking, streaming, headless browser?)
- Remaining docs: prior-art.md, hybrid-loops.md
- Memory system: `docs/design/memory.md` — SQLite-backed `store/recall/forget`. Deferred until concrete use case.
- Jinja2 grammar publish: NOT via arborium (they vendor their own); publish as our own crate (`tree-sitter-jinja2` name taken — pick another). Update normalize-grammars dep. Local `grammars/jinja2/` + `find_local_grammars()` in xtask is sufficient for now.
- view: directory output shows dir name as first line (tree style) - intentional?

## Implementation Notes

### Self-update (`normalize update`)
- Now in commands/update.rs
- GITHUB_REPO constant → "rhi-zone/normalize"
- Custom SHA256 implementation (Sha256 struct)
- Expects GitHub release with SHA256SUMS.txt
