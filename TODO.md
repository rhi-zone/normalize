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
- [x] File-level dependency tracking (import graph edges to scope fact re-evaluation) — `WatchedRoot.rev_deps` in daemon, `affected = changed ∪ reverse-deps`, `last_affected` stored for Datalog integration
- [x] `normalize watch` CLI (expose daemon file-watching with TUI output)
- [x] Progress reporting for `structure rebuild` (indicatif bars for file scan, symbol parsing, index storage)
- [x] Progress reporting for `analyze duplicates`, `analyze architecture`, `analyze duplicate-types` (indicatif bars for file processing, spinners for architecture phases)

---

## P1 — Short-term Improvements (coherence / usability)

### Agent UX: comprehensive compact output audit

Baseline audit in `docs/agent-ux-audit.md` (2026-03-21) covers 12 commands across 3 models (Haiku, Sonnet, Opus). Quick wins fixed. Remaining work:

- [x] `analyze health --json` and `analyze summary --json` bloated by unbounded `large_files` (180KB+) → added `--limit` (default 10, 0 = no limit) to `analyze health`
- [x] `syntax ast --json` produces 673KB for a 200-line file → added `--depth` flag (default -1 = unlimited) that truncates the CST; `--compact` now shows a node-type outline instead of full dump
- [x] `syntax query --compact` only showed match count → now shows one line per match: `path:line: @capture = text`
- [x] `rules show --json` and `rules tags --json` returned `{message: string, success: bool}` text blobs → now return `RuleInfoReport` (structured fields: id, severity, enabled, tags, languages, message, fix, description, allow) and `RulesTagsReport` (`{tags: [{tag, source, count, rules}]}`)
- [x] `daemon list` returned exit 1 + stderr when daemon not running → now returns exit 0 with `{running: false, roots: []}` (only exits 1 on actual errors like socket permission denied)
- [x] `package list --json` silently dropped multi-ecosystem advisory → `PackageListReport` now includes `ecosystems_detected: Vec<String>`; when multiple ecosystems exist, agents see all names and know to re-run with `--ecosystem` for complete results
- [x] `analyze architecture --compact` showed only cross-imports (hubs/layers hidden when empty) → `format_text()` now emits compact tagged lines (`HUBS:`, `LAYERS:`, `COUPLING:`, `SYMBOLS:`, `ORPHANS:`, `SUMMARY:`) always, even when empty; `format_pretty()` retains the original tabular layout
- [x] Run full audit pass over all ~30+ subcommands — Pass 2 committed 2026-03-26, 37 commands evaluated
- [ ] Re-run multi-model audit after fixes to verify improvement

### ~~Main Crate Responsibility Boundaries~~ (audited 2026-03-15 — no action needed)

Crate split is correct. All 38 published crates justified. No reusable logic trapped in `normalize`; no unjustified extractions. Single-consumer domain libraries (graph, scope, edit, deps, etc.) are correctly placed — the test is "CLI wiring vs. domain logic", not "has 2+ consumers". Revisit only if a concrete second consumer appears for a specific module.

### Analyze Command Consolidation — remaining work

**Current: ~38 commands** (after 2026-03-15/16 consolidation: deleted `analyze parse`, `analyze query`, `analyze all`, `analyze node-types` → moved to `syntax`; merged 4 trend commands; deleted `normalize-rules-loader`).

**Phase 3 rank infrastructure (done 2026-03-12):**
- `RankEntry` trait + `Column`/`Align` + `format_ranked_table()` in `normalize-analyze::ranked`
- Migrated 13 commands to shared tabular rendering
- `DiffableRankEntry` + `--diff` on all 12 rank commands

**Future (low priority):** `security` → SARIF rules engine. `docs`/`security` → rules migration (~-3 commands).

---

### `analyze` Architecture Redesign (high priority)

**Done (2026-03-16):** `normalize rank` introduced with 20+ commands migrated from `analyze`. Graph navigation (`call_graph`, `trace`, `dependents`, `provenance`) folded into `view` (`referenced-by`, `references`, `dependents`, `trace`, `graph`, `history`, `blame`). `ViewOutput` enum dissolved; `ViewReport`/`ViewNode` unified. `view list` added.

**Remaining in `analyze`:** trends (complexity-trend, length-trend, density-trend, trend), git history (activity, coupling-clusters, repo-coupling, cross-repo-health), big-picture (health, architecture, summary, all), plus docs/security/test-gaps/skeleton-diff. These stay until a clear home emerges.

**Not yet decided:**
- Where big-picture commands live (`architecture`, `summary`, `health`) — synthesized understanding, not ranking, not navigation. No trait identified yet.
- Whether `analyze` dissolves entirely or gets a new identity — will become clear over time.

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
- ~~Debug-print rules: Go (`fmt-print`) and Python (`print-debug`) enabled by default, but C/C++/Java/Kotlin/PHP/Rust/Swift/C# equivalents all disabled. Should be consistent.~~ Resolved: all debug-print rules were already consistently disabled by the frictionless OOTB change; Go/Python/JS only enabled via local config override for dogfooding.
- ~~Correctness rules that should be enabled by default: `go/defer-in-loop`, `go/sync-mutex-copied`, `swift/force-unwrap`, `python/raise-without-from`, `python/use-with`, `ruby/method-missing`.~~ Done: all 6 enabled by default with `recommended = true`.
- ~~Potentially too aggressive defaults: `rust/chained-if-let` (error severity for style).~~ Done: downgraded to warning. `rust/numeric-type-annotation` already disabled. Tuple-return rules already info-level and disabled.

**Wizard UX improvements:**
- [x] Show rules with zero violations (summary count + pointer to `rules list`)
- [x] `recommended = true` frontmatter for genuine bug/correctness rules vs style opinions
- [x] Recommended rules shown first in wizard (sorted before violation count)
- [x] Standalone `normalize rules setup` command (don't require re-running `init`)
- [x] Group by tag/category instead of flat violation-count sort
- [x] Add batch operations: "enable all [correctness] rules", "disable all [style] rules"
- [x] Show practical impact: "2 violations (quick fix)" vs "847 violations (major cleanup)"

### SARIF engine actionable output

- ~~`rules run --engine sarif` could show which SARIF tools had errors (not done)~~ Done: tool errors captured in `DiagnosticsReport.tool_errors` and shown in text/pretty output

### Duplicate/clone detection improvements

- [x] Per-subcommand excludes in config: `[analyze.duplicates] exclude = [...]` via `#[serde(flatten)]` HashMap on `AnalyzeConfig`. Wired into all analyze subcommands that accept `--exclude`: duplicates, complexity, length, docs, health, all, test-gaps, uniqueness, hotspots, files, size, coupling, coupling-clusters, ownership, fragments, skeleton-diff.
- [x] "Parallel impl directory" heuristic: if >=5 pairs originate from the same directory pair, fold them into a suppressed note (e.g., "388 pairs suppressed across 10 directory groups"). Applied to exact-functions, similar-functions, and similar-blocks when `!include_trait_impls`. Handles 2-location pairs and multi-location groups (up to 2 distinct directories).
- [x] `similar-functions`: body-pattern cluster suppression — connected-component analysis on pair graph; components spanning 3+ files with 5+ pairs are suppressed as `SuppressedBodyPatternGroup`. Catches Language trait impl case (e.g. `extract_imports` across 20 structs). Applied when `!include_trait_impls`. `suppress_widespread_body_patterns()` in `duplicates.rs`.
- ~~Consider min-lines bump for `similar-blocks` (currently 10)~~ **Done**: bumped default to 15 for similar-blocks, made configurable via `--min-lines` CLI flag and `[analyze.duplicates] min_lines` in config

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

### ~~normalize-ratchet: metric regression tracking~~ (done 2026-03-22)

- [x] `normalize-metrics` crate: shared `Metric` trait, `MetricFactory`, `Aggregate` enum + `aggregate()`, `filter_by_prefix()` — depended on by both ratchet and budget
- [x] `normalize-ratchet` crate with 6 metrics: complexity, call-complexity, line-count, function-count, class-count, comment-line-count
- [x] 6 CLI commands (behind `cli` feature): `measure`, `add`, `check`, `update`, `show`, `remove`
- [x] Entries are `(path, metric, aggregation) → value`; path can be dir/file/symbol (`file/Parent/fn`); always aggregated — symbol path is degenerate case (n=1)
- [x] Baseline stored in `.normalize/ratchet.json`; 6 aggregation strategies (mean/median/max/min/sum/count); defaults configurable via `[ratchet]` / `[ratchet.metrics.<name>]` in `.normalize/config.toml`
- [x] `MetricFactory` type alias outside `cli` feature; `RatchetConfig` wired into `NormalizeConfig` via `#[param(nested, serde)]`
- [x] Native rules integration: `normalize rules run` detects regressions via `ratchet/<metric>` rule IDs
- [x] `--base <git-ref>` on `check` and `measure` for historical comparison via git worktrees
- [x] `normalize-budget` crate: diff-based budget system; each entry has `(path, metric, aggregate, ref) → {max_added, max_removed, max_total, max_net}` (all optional); budget stored in `.normalize/budget.json`
- [x] 7 diff metrics: lines, functions, classes, modules, todos, complexity-delta, dependencies
- [x] Native rules integration: `budget/<metric>` rule IDs alongside ratchet rules

**Follow-up ideas (not planned):**
- `--base` worktree approach is correct but slow for large repos; could cache measurements per git-ref in `.normalize/ratchet-cache/`
- Call-graph BFS is intra-project only (no cross-crate edges); future: integrate with `normalize-graph` if cross-crate call data exists
- Trend charts (`normalize ratchet trend`) could visualize metric history over git log

### ~~CI readiness~~ (done — 0.2.0 shipped)

- [x] `normalize ci` command — `--no-syntax`/`--no-native`/`--no-fact`/`--strict`/`--sarif` flags, structured output, non-zero exit on errors.
- [x] Install script — `install.sh` + `install.ps1`, platform/arch detection, SHA256 verification, version pinning via `NORMALIZE_VERSION`.
- [x] CI documentation — `docs/ci.md` with GitHub Actions/GitLab/CircleCI snippets.
- [x] Version bump to 0.2.0 — all 38 published crates bumped; `normalize update` works against GitHub releases.
- [x] Polish pass — `--help` audit, exit codes verified, smoke-tested on external repos.

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
- [x] ~~All rules (builtin + user) compile to dylibs via Ascent + `abi_stable`~~ — abandoned: dylib approach caused heap corruption (`corrupted double-linked list`) from `RString/RVec` allocator mismatch across dylib boundary. Replaced with interpreted `.dl` files via `normalize-facts-rules-interpret` (no dylib loading at all).
- [x] Same infrastructure for both - builtins ship pre-compiled, users compile theirs (done via `.dl` files)
- [x] Same syntax for both (rules can graduate from user to builtin) — done: `.dl` files for all rules
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
- [x] ~~Self-install builtin dylib~~ — no longer applicable; builtins are embedded `.dl` files in `normalize-facts-rules-interpret/src/builtin_dl/`, no dylib or copy step needed.

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

### 0.2.0 — "Coherent surface, reliable index"

**Theme:** The CLI is small enough to hold in working memory. The index is reliable enough
to depend on. The LSP is useful day-to-day.

**Already done (since 0.1.0):**
- Qualified import resolution + `callee_resolved_file` in facts (schema v6)
- Two-tier LSP diagnostics (immediate syntax, debounced fact rules)
- Incremental index updates on save (`FileIndex::update_file()`)
- Compiled query caching in `GrammarLoader`
- Language coverage audit: 47/84 languages at 100% .scm coverage; all feasible gaps filled
- `RankEntry` trait + `format_ranked_table()` — shared rendering for 13 rank commands
- `--diff <ref>` on all 12 rank commands
- Progress bars for `structure rebuild`, `analyze duplicates`, `analyze architecture`
- Per-subcommand excludes in config
- `rules recommended` field + `normalize init --setup` interactive wizard (tag grouping, impact labels, batch ops — done 2026-03-15)
- 30+ new syntax rules (Java, C/C++, C#, Kotlin, Swift, PHP)
- [x] `normalize daemon watch` — file change streaming (2026-03-15)
- [x] File-level reverse-dep tracking in daemon (2026-03-15)
- [x] Duplicate detection: cross-file same-body-pattern suppression (2026-03-15)
- [x] `normalize-rules-loader` folded into `normalize-rules` (2026-03-15)
- [x] `normalize rank` introduced; 20 ranked commands migrated from `analyze` (2026-03-16)
- [x] `normalize analyze all` deleted; `analyze node-types` moved to `syntax` (2026-03-16)
- [x] 4 trend commands merged into `normalize analyze trend-metric --metric <...>` (2026-03-16)
- [x] CLI command organization ADR written (2026-03-16)

**Remaining before 0.2.0:**

*LSP / index (from P0):*
- [x] Wire incremental Datalog evaluation: `CachedRuleEngine`, `prime_rule_engine`,
  `run_rule_incremental`, and `run_rule_with_cache` added to `normalize-facts-rules-interpret`.
  `collect_fact_diagnostics_incremental` in `normalize-rules/src/runner.rs` uses a process-level
  `ENGINE_CACHE` (keyed by root + rule_id) to retract only changed-file facts and re-derive.
  Daemon integration point documented with a TODO comment in `daemon.rs::refresh_root` pointing
  to `collect_fact_diagnostics_incremental` with `Some(&watched.last_affected)`.
  Daemon wired: after each index refresh, daemon calls `collect_fact_diagnostics_incremental`
  with `Some(&watched.last_affected)` to warm the `ENGINE_CACHE` — next `normalize ci` or
  `normalize rules run` in the same process uses the incremental path automatically.
  **Remaining:** fix JIT string comparison bug in ascent-interpreter to make eval fast.
- [ ] Fix JIT string comparison bug in ascent-interpreter and re-enable `SharedJitCompiler`
  in `run_rules_source` / `run_rules_batch`. **Release blocker: ascent-interpreter is our own
  project — this is fixable on our timeline. Incremental eval reduces re-derivation scope;
  JIT makes the derivation itself fast. Both are needed for CI performance on large repos.**

*CLI surface (from P1):*
- [x] `view` refactor phase 1: graph navigation + history as subcommands — done 2026-03-16
- [x] `view` refactor phase 2: dissolve `ViewOutput` enum into `ViewReport` + `view list` — done 2026-03-16

*CI readiness (from P1 — see "CI readiness" section above):*
- [x] `normalize ci` command — `--no-syntax/native/fact`, `--strict`, `--sarif`, `-p <path>`, graceful index-not-built handling
- [x] Install script (curl | sh) — SHA256 verification, `NORMALIZE_VERSION` pinning, `~/.local/bin` default
- [x] CI documentation (`docs/ci.md`) — GitHub Actions/GitLab snippets, ratchet bootstrap workflow, SARIF output
- [x] Polish pass — version string, --help accuracy, config parse warning, view error messages,
  stale --engine flag references in docs. **Exit code 1 vs 2 differentiation (violations vs
  setup errors) requires server-less `ExitCode`-carrying error type — deferred post-0.2.0.**

*Release mechanics:*
- [x] Bump all crate versions to 0.2.0 in Cargo.toml files
- [x] Tag and push `v0.2.0`
- [ ] Verify `normalize update` works against a real GitHub release (cross-platform smoke test)

**Not blocking 0.2.0:**
- Comprehensive language fixtures (explicitly long-term)
- Semantic rules system (separate infrastructure, post-0.2.0)
- Shadow worktree / namespace-qualified lookups (low priority)

---

### 0.3.0 — "Understand and refactor, fast"

**Theme:** normalize becomes the tool you reach for when you need to understand a codebase
and make a cross-cutting change safely. The index is no longer just for analysis — it backs
precise multi-file edits. Linting grows a semantic tier (18 fact rules already exist; the
gap is polish + new rules, not infrastructure). Everything is incremental: no cold rebuilds,
no full re-evaluations on every invocation.

**Pillar 4 — Incremental everything**

The daemon is running but CLI invocations don't route through it — every `normalize rules run`
is a cold eval, every `structure rebuild` re-indexes the world. The goal: make the fast path
the default path.

- [ ] **Incremental index** — on `structure rebuild`, only re-index files changed since the
  last build (mtime/hash based). Full rebuild only when schema changes or forced with `--full`.
- [ ] **CLI → daemon routing** — `normalize rules run` (and `normalize ci`) should talk to
  the running daemon and get the pre-warmed Datalog cache instead of cold-evaluating. If no
  daemon is running, fall back to cold eval transparently.
- [ ] **Incremental syntax rules** — currently no incremental path; only re-run queries on
  files that changed since last run. Cache results keyed by file hash.
- [ ] **Incremental native rules** — stale-summary, broken-ref, ratchet, budget checks should
  skip files whose content and deps haven't changed.
- [ ] **Persistent query cache** — store per-file tree-sitter query results in the SQLite index
  so repeated `normalize view`, `normalize rank`, etc. don't re-parse unchanged files.

**Pillar 5 — Perf and memory baseline**

Before optimizing, measure. We have no perf benchmarks and no memory budget. The malloc
crash in the pre-commit hook is a warning sign.

- [x] Establish wall-clock benchmarks for the hot paths: `structure rebuild`, `rules run`,
  `view`, `rank`. Run against a mid-size real repo (normalize itself is a good target).
  Track in CI so regressions are caught. (`benches/` crate added with criterion; baseline TBD values in `docs/perf-baseline.md`)
- [ ] Profile memory usage of a full `structure rebuild` and `rules run --engine fact` on
  normalize. Identify the top allocators. The pre-commit malloc crash suggests at least one
  path has unbounded allocation.
- [ ] Set memory budgets per command and enforce them in tests (e.g. `structure rebuild`
  on normalize should not exceed N MB RSS).

**Current state (post-0.2.0 audit):**
- `normalize view` already has full graph navigation: `referenced-by`, `references`,
  `dependents`, `trace`, `graph` are wired and functional. The "fold call-graph into view"
  item from the 0.2.0 design is already done.
- `commands/find_references.rs` exists but is not exposed as `normalize refs` — just needs
  wiring.
- 18 fact rules (Datalog) already exist: `circular-deps`, `dead-api`, `unused-import`,
  `god-file`, `god-class`, `orphan-file`, `duplicate-symbol`, `fan-out`, `hub-file`,
  `layering-violation`, `long-function`, and more. Semantic rules infrastructure is mature.
- `normalize-facts-rules-builtins/src/circular_deps.rs` (compiled Ascent macro) may be
  dead code — the Datalog version in `builtin_dl/circular_deps.dl` is what runs. Audit and
  remove if so.
- Incremental evaluation API (`run_rules_source_incremental`) is implemented but not wired
  into any CLI call path. JIT disabled pending upstream string comparison bug fix.

**Pillar 1 — `analyze` dissolution**

`normalize view` has absorbed graph navigation. `normalize rank` has absorbed 21 ranking
commands. `analyze` still hosts 19 commands that don't fit either:

- [ ] Trend commands (`complexity-trend`, `length-trend`, `density-trend`, `test-ratio-trend`)
  — time-series of ranking metrics. Fold into `rank` with `--trend` flag or dedicated
  `normalize trend` subcommand.
- [ ] Synthesis commands (`architecture`, `summary`, `health`, `coupling-clusters`,
  `cross-repo-health`) — big-picture, not a ranked list. Find the unifying trait or leave
  in `analyze` until the pattern is clear. Don't force a home.
- [ ] Residual commands (`activity`, `docs`, `security`, `test-gaps`, `skeleton-diff`,
  `repo-coupling`, `node-types`, `length`, `all`) — audit each: belongs in rank/view/rules
  or stays as standalone?
- [ ] Once all commands have a proper home, `analyze` dissolves. Don't rush this — clarity
  matters more than speed.

**Pillar 2 — Semantic refactoring**

Building blocks are all present. The gap is composition:

- [x] `normalize refs` absorbed into `view referenced-by` — `CallEntry.access:
  Option<String>` field added (values: `"read"`/`"write"`/`"read-write"`); currently
  always `None` pending index + scope engine changes below.
- [x] **Populate `access` in `CallEntry`** — `calls` table has `access TEXT` column (schema v7); `@call.write` capture in Rust `.scm` files populates it; `view referenced-by` displays `[read]`/`[write]`/`[read-write]`. Other languages: extend `.scm` files when grammars support write-position detection.
- [ ] `normalize rename <target> <new-name>` — cross-file symbol rename. Uses
  `view referenced-by` to find all sites, normalize-scope for shadow/conflict detection,
  batch edit for atomic multi-file rewrite, shadow git for preview. `--dry-run` shows
  diff, no writes. This is the highest-value refactoring command.
- [ ] `normalize move <target> <destination>` — move a symbol to another file, updating all
  import sites. Requires rename infrastructure + import rewriting. After rename lands.
- [ ] `normalize extract <file:start-end> <new-name>` — extract a region into a new function,
  rewriting the call site. Single-file first; cross-file as stretch.
- [ ] `normalize inline <target>` — inline a single-use function or constant. Single-file.
- [ ] Post-edit index invalidation: after a multi-file edit, mark affected files dirty in the
  daemon's reverse-dep graph so the index refreshes without a full rebuild.

**Pillar 3 — Semantic rules (stretch goal)**

18 fact rules already exist and run via `--engine fact`. The gap is new rules and wiring
incremental evaluation so they're fast enough for pre-commit use:

- [ ] Audit and remove `normalize-facts-rules-builtins/src/circular_deps.rs` if dead code
  (compiled Ascent macro superseded by Datalog version).
- [ ] New fact rules: `dead-parameter` (param never read in any call path, needs scope),
  `missing-test` (exported function with no test calling it), `stale-mock` (test mock
  references a function that no longer exists).
- JIT fix and incremental eval wiring moved to 0.2.0 blockers.

**Rules engine architecture — drop abi_stable, external process + rkyv for custom rules**

The current dylib rule pack system (`libloading` + `abi_stable` + `RString`/`RVec` in
`Relations`) has a heap corruption bug (glibc "corrupted double-linked list" on
`normalize rules run --type fact`) caused by allocator boundary mismatch between the
main binary and loaded `.so` files. This is not a patch-sized fix — the design is wrong.

Target architecture:

| Rule kind | Boundary | Serialization |
|---|---|---|
| Built-in native (stale-summary, broken-ref, …) | None — compiled in | — |
| Datalog (builtin + user `.dl` files) | None — JIT in-process | — |
| Custom native Rust rules | External process | rkyv |
| Heavy external tools | External process | JSON / SARIF |

rkyv for custom native rules: the external process receives `Relations` as a zero-copy
rkyv archive (mmap or pipe), does its computation, writes diagnostics back. This gives
external-process safety (no allocator boundary, no ABI concerns) without paying full JSON
serialization cost — cheap enough for pre-commit. SARIF stays for heavy tools where JSON
overhead is acceptable.

- [x] Drop `libloading`, `abi_stable`, `loader.rs` and the dylib search-path machinery. (commit 398b715b)
- [x] Replace `RString`/`RVec` in `Relations` with plain `String`/`Vec`. Fixes heap corruption.
- [ ] Add `rkyv` derive to `Relations` + fact types for the external-process boundary.
- [ ] Define the external native rule protocol: receive rkyv Relations on stdin, write
  NDJSON diagnostics on stdout. Document in `docs/rules.md`.

**Dependencies / preconditions:**
- `normalize refs` ships first — it's the foundation for rename, move, and dead-parameter rule.
- Incremental Datalog wiring can happen independently of new rules.
- JIT fix is upstream; don't block anything on it.
- abi_stable removal can land independently of JIT — unblock it first.

**Pillar 6 — Discoverability (every context type expressible in one call)**

The design principle: every useful type of context around a symbol or file should be
expressible as a single normalize call, not a sequence of greps and reads. This doesn't
mean every type will be *used* often — but it should *exist* as an option. Agents and
users opt in when they need it; the absence of an option forces multi-call workarounds
regardless of how rarely the context is useful.

The work is mechanical and inevitable — every context type will need to exist eventually,
so build them. Mine Claude Code session logs (adapt `scripts/session-corrections.sh` for
command-sequence analysis) to find what's *still missing* after the obvious types are
covered, not to decide what to build first.

Context types that should exist (independent of priority):
- **Blast radius**: "if I change X, what breaks?" — forward reachability (callers,
  importers, dependents). The dual: "why is X broken?" — backward reachability (what X
  depends on, its call chain). Both are index queries; neither is expressible today in
  one call without multiple `--referenced-by`/`--references`/`--graph` round-trips.
- **Directory orientation**: `normalize view <dir>` surfacing `SUMMARY.md` as a preamble
  and `//!` module docs for files. Agents get orientation + symbols without a separate
  read.
- **Change impact**: given a diff or set of changed files, what symbols/rules are affected?
  Feeds into incremental eval (Pillar 4) and debugging alike.

Concrete unblocked items:
- [x] `normalize view <directory>` surfaces `SUMMARY.md` as preamble; `--json` adds `"summary"` field.
- [x] `rust/missing-module-doc` syntax rule — `lib.rs`/`mod.rs` files with no `//!`.
- [x] Split `stale-summary` into `missing-summary` (presence) + `stale-summary` (freshness), each with `paths` glob config.
- [ ] `normalize view <file>` surfaces `//!` crate/module docs and equivalents for all languages.

**Not targeting 0.3.0:**
- Full AST rewriting (tree-sitter edit API, round-trip fidelity)
- Type-aware refactoring (normalize has no type resolver)
- Jinja2 grammar crate publish

---

## Post-polish review

After the fixpoint polish loop reaches 0 findings, do a retrospective pass:
review all the changes made during the polish loop and evaluate whether they
were actually helpful. Some fixes may have been mechanical (rename, doc comment)
with clear value; others may have introduced complexity or changed semantics in
ways worth questioning. Candidates to review: catch_unwind in FFI (does it hide
real bugs?), load_rules_config merge (was the old behavior intentional?),
find_cycles_dfs iterative conversion (was stack depth ever actually a problem?).

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
