# Normalize Roadmap

See `CHANGELOG.md` for completed work. See `docs/` for design docs.

## Next Up

- [x] Fixture-based tests for all syntax rules (`normalize-syntax-rules`):
  - `tests/fixtures/<lang>/<rule-name>/match.<ext>` — must produce ≥1 findings
  - `tests/fixtures/<lang>/<rule-name>/no_match.<ext>` — must produce 0 findings
  - Single `tests/rule_fixtures.rs` test runner (language-agnostic, no Rust per rule)
  - **New builtin rules must include fixture files** — runner silently skips missing ones

- [x] Add rule writing guide (`docs/syntax-rules.md`) and link from `docs/cli/rules.md`
- [x] Rule sharing/import: `normalize rules add/update/list/remove` (Phase 1 complete)
- [x] Auto-fix support: `normalize analyze rules --fix` with fix templates
- [x] Expand #[cfg(test)] detection for Rust rules (rust.is_test_file)

## Next Up

### Git Analysis Enhancements (`analyze hotspots`)

Current `analyze hotspots` is file-level churn only (`commits × √churn`). Enhance with:

- [x] **Hotspot × complexity**: weight churn by cyclomatic complexity from index. Score: `commits × √churn × log₂(1 + complexity)`.
- [x] **Temporal coupling**: `analyze coupling` — files that change together in the same commits (co-change analysis).
- [x] **Blame hotspots**: `analyze ownership` — ownership concentration per file via `git blame`. Bus factor, top author percentage.
- [x] **Recency weighting**: `--recency` flag — exponential decay (180-day half-life), recent changes weighted higher.

### Cross-Repo Analysis

Analyze across multiple repositories — activity trends, shared patterns, inter-repo dependencies. Operates on a directory of sibling repos or a configured workspace.

**Analysis:**
- [ ] **Activity over time**: per-repo commit volume, author focus, churn over configurable time windows. "Which repos are active? Which are stagnating? Where is energy going?"
- [x] **Inter-repo dependency graph**: which repos import/depend on which (via package manifests: Cargo.toml deps, package.json, go.mod). Visualize the cross-repo architecture. → `analyze repo-coupling`
- [ ] **Cross-repo duplicates**: find shared code across repos that should be a library. Extend `duplicate-functions`/`similar-functions` to work across repo boundaries.
- [ ] **Cross-repo hotspots**: aggregate churn/complexity/coupling across repos. Which repo has the most tech debt?
- [ ] **Cross-repo ownership**: who works on what across the org. Author overlap between repos.

**Commands:**
- [ ] **Run commands across repos**: `normalize --repos ~/git/org/ tools lint`, `normalize --repos ~/git/org/ analyze hotspots`. Discover projects, run in parallel, aggregate output.
- [x] **Cross-repo coupling**: repos that get commits in the same time window (e.g., same day/PR). Indicates hidden cross-repo dependencies. → `analyze repo-coupling`

**Design considerations:**
- Discovery: `--repos <dir>` scans for `.git` dirs, or `normalize.workspace.toml` lists repos explicitly
- Output: per-repo breakdown + aggregate summary
- Incremental: cache per-repo results, only re-analyze changed repos

## Remaining Work
- `normalize view` symbol not found: show all candidate symbols with **trigram containment ≥ 0.6** against the query (skip if query < 4 chars). Metric: `|trigrams(query) ∩ trigrams(candidate)| / |trigrams(query)|` — asymmetric by design, measures how much of the query appears in the candidate.
  - Handles prefix typing (`cmd_dup` → all 5 query trigrams appear in `cmd_duplicate_functions_with_count` → 1.0 ✓)
  - Handles interior substrings (`duplicate_functions` → high containment ✓)
  - Handles light typos (`duplikat_funcs` → shared {dup,upl,pli,_fu,fun,unc} = 6/12 = 0.5 ✓; note: `lik/ika/kat` don't match because "duplicate" has `lic/ica/cat` not `lik/ika/kat`, and `at_/t_f` miss because "duplicate" ends `ate_` not `at_`)
  - Why not edit distance: fails on length difference (short query vs long name always scores poorly even if it's a good prefix match)
  - Why not Jaccard: same asymmetry problem — short query vs long name gives tiny union, low score
  - Why not substring: misses any typo
  - Why not word-token prefix: misses inter-token typos, requires exact token boundaries
  - Threshold 0.5 and min-length 4; false positives are cheap (suggestions shown only on failure)
- Namespace-qualified lookups: `normalize view std::vector`, `normalize view com.example.Foo`
  - Requires language-specific namespace semantics - low priority
- Shadow worktree: true shadow-first mode (edit in shadow, then apply)
  - Current: --shadow flag works, but not default for all edits
  - Zero user interruption (user can edit while agent tests in background)

### Configuration System
Sections: `[daemon]`, `[index]`, `[aliases]`, `[view]`, `[analyze]`, `[text-search]`, `[pretty]`, `[serve]`

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

Crates with registries:
- [x] normalize-languages: `Language` trait, `register()` in registry.rs
- [x] normalize-language-meta: `Capabilities` struct, `register()` for user overrides
- [x] normalize-cli-parser: `CliFormat` trait, `register()` in formats/mod.rs
- [x] normalize-chat-sessions: `LogFormat` trait, `register()` in formats/mod.rs
- [x] normalize-tools: `Tool` trait (`register_tool()`), `TestRunner` trait (`register()`)
- [x] normalize-ecosystems: `Ecosystem` trait, `register_ecosystem()` in ecosystems/mod.rs
- [x] normalize-openapi: `OpenApiClientGenerator` trait, `register()` in lib.rs
- [x] normalize-typegen: `Backend` trait, `register_backend()` in registry.rs

Pattern: traits are the extensibility mechanism. Users implement traits in their own code, register at runtime. normalize CLI can add Lua bindings at application layer for scripting.

### CLI API Consistency
Audit found fragmentation across commands. Fix for consistent UX:

**High priority:** (DONE)
- [x] `--exclude`/`--only` parsing: unified to comma-delimited across all commands
- [x] Output flags in `analyze`: removed local flags, uses root-level `--json`/`--jq`/`--pretty`/`--compact`
- [x] Short flag `-n` collision: changed to `-l` for `--limit` (consistent with sessions)
- [x] `--root` vs `--project`: sessions now uses `--root` like other commands
- [x] `--jq` semantics: documented - root filters whole JSON, sessions filters per-line (JSONL) - intentional

**Medium priority:**
- [x] Subcommand defaults: reviewed - intentional design (commands with clear primary action default to it, e.g., lint→run, test→run, analyze→health; commands with no clear primary require explicit, e.g., package, index)
- [x] `--allow` semantics: reviewed - intentional (different analysis types need different allowlist formats: patterns for files/hotspots, locations for duplicate-functions, pairs for duplicate-types; help text documents each)
- [x] `--type` vs `--kind`: standardized to `--kind` (view now uses `--kind` like analyze complexity)

**Programmatic CLI Interface (in progress):**
- [x] `--jsonl`: JSON Lines output (arrays emit one object per line, scalars emit single line)
- [x] `--output-schema`: output JSON schema for command's return type
- [x] `--input-schema`: output JSON schema for command's input arguments
- [x] `--params-json`: pass command arguments as JSON (overrides CLI flags)
- [x] Core infrastructure: `OutputFormatter` requires `JsonSchema`, all output types derive it
- [x] `--output-schema` wired up for: aliases, text-search, analyze, grammars, sessions, tools, context, history
- [x] `--input-schema` + `--params-json` wired up for: aliases, text-search, analyze, sessions, view, history, context, edit, init, generate, translate, grammars, tools
- [x] `--jsonl` + `--jq` combination (apply jq filter, then emit results as jsonl)
- [ ] Wire up `--output-schema` for: view (10+ implicit modes — needs dedicated refactor pass)

### CLI Internal Consolidation
Eliminate the `cmd_*` middle layer. Commands should be library functions that return `Result<T>`, with CLI plumbing auto-generated.

**Current problem:** Three layers where two suffice:
1. `run()` — CLI boilerplate (schema, params-json, config merge, exit codes). Identical across commands.
2. `cmd_text_search()` — builds a filter, calls library fn, prints result, returns exit code. Barely needed.
3. `text_search::grep()` — the actual library function.

Layer 2 should not exist. Layer 1 should be generic.

**Target:** Use `server-less` `#[cli]` macro (github:rhi-zone/server-less) to generate CLI from typed methods. Commands become plain library functions, macro handles all plumbing:
```rust
#[cli(name = "normalize")]
impl NormalizeService {
    /// Search file contents for a pattern
    pub fn grep(&self, pattern: String, root: Option<PathBuf>, ...) -> Result<GrepResult, Error> {
        text_search::grep(...)
    }
}
```
This eliminates: per-command `Args` structs, `run()` boilerplate, `cmd_*` middle layer, duplicated `--root`/`--exclude`/`--only` definitions.

**server-less `#[cli]` status** (rhi-zone/server-less):
- [x] `bool` params as switches (`ArgAction::SetTrue`)
- [x] `Vec<T>` params with Append + comma delimiter
- [x] Global/shared flags (`global = [...]` + built-in `--json`/`--jsonl`/`--jq`)
- [x] `defaults = "fn_name"` hook (bridge point for config file loading)
- [x] `--output-schema` / `--input-schema` / `--params-json` support (full override, JSON string arg)

**Steps (normalize side):**
- [x] Add `server-less` dependency with `cli` feature
- [x] Wire `OutputFormatter`: `display_with` bridges to `format_pretty()`/`format_text()` via `Cell<bool>` for pretty/compact globals. server-less handles JSON/JSONL/JQ before `display_with`.
- [x] Wire `defaults` hook to `NormalizeConfig` loading (config file merge)
- [x] Migrate one simple command (text-search → `grep`) as proof of concept
- [x] Delete `cmd_text_search`, `TextSearchArgs`, `text_search::run()` — replaced by `#[cli]` on method
- [x] Remove legacy `text-search` command (fully replaced by `grep`)
- [x] Migrate `aliases`, `context`, `init` to server-less (Phase 3)
- [x] Migrate remaining commands, deleting `cmd_*` functions and manual Args structs
  - Done: `update`, `translate`, `daemon`, `grammars`, `generate` (Batch 1)
  - Done: `facts`, `rules`, `package` (Batch 2)
  - Done: `history`, `sessions`, `tools`, `edit` (Batch 3)
  - Done: `view` — extracted `build_view_service()` + `build_view_*_service()` per mode, `ViewResult` wrapper for text+JSON, service method with `display_view`.
  - Deferred: `analyze` — 29 subcommands, deep config/allowlist/filter integration. Many subcommands already return `OutputFormatter` types; extract `build_*` functions and wire to `AnalyzeService`.
  - Deferred: `serve` — long-running servers (MCP, HTTP, LSP), no structured return type
- [ ] Final cleanup (after view/analyze migrated): delete `Commands` enum, `Cli` struct, `HELP_STYLES`, `help_color_choice()`, remove clap from normalize crate deps. `Commands` now has only 3 entries (View, Analyze, Serve).
- [ ] Centralize multi-repo dispatch logic (currently hardcoded in main.rs for specific analyze subcommands)
- [ ] Audit whether any of the 19 top-level subcommands should be merged or nested differently

### CLI Cleanup
- [x] Move `normalize plans` to `normalize sessions plans`: groups tool-specific data under sessions
- [x] Rename `normalize filter aliases` to `normalize aliases`: removes unnecessary namespace layer
- [x] Unify `lint`/`test` under `normalize tools`: `normalize tools lint [run|list]`, `normalize tools test [run|list]`
- [x] Remove `analyze lint`: duplicate of `normalize lint`, adds no value
- [x] Unify `normalize rules` as umbrella for all rule types:
  - `normalize rules list` — lists ALL rules (syntax + fact, builtin + user), with `--type` filter
  - `normalize rules add` — adds rules (detects `.scm` vs `.dl` by extension)
  - `normalize rules run` — runs all rules, with `--type` filter
  - `facts check` delegates to unified rules infrastructure
  - Severity model unified: both use `Severity` enum (error/warning/info), `deny` backward-compat mapped

### Documentation Cleanup
- [x] Comprehensive docs audit: run each command's `--help` and compare against `docs/`. Known gaps:
  - ~~Fact rules (interpreted + compiled): zero user-facing docs~~ → `docs/fact-rules.md`
  - ~~`facts` subcommands (`rebuild`, `files`, `packages`, `check`, `rules`): undocumented~~ → `docs/cli/facts.md`
  - ~~CLI drift from refactoring (renames, moved subcommands, new flags)~~ → fixed `index`→`facts` in commands.md, README
  - ~~Need fact rules writing guide equivalent to `docs/syntax-rules.md` for syntax rules~~ → `docs/fact-rules.md`
- [x] Remove `normalize @` and `normalize workflow` references from docs - spore handles workflow running now
  - Archived: script.md, agent*.md, lua-cli.md, agent-state-machine.md, workflow-format.md, agent-commands.md, lua-api.md, agent-dogfooding.md
  - Updated: shadow-git.md, log-analysis.md, workflows/README.md, security-audit.md, dogfooding.md, langgraph-evaluation.md, prior-art.md
  - Kept: normalize-chat-sessions parser (format still valid for reading old logs)

### Rust Redesign Candidates
- Rules engine: consider semgrep/ruff integration instead of custom
- Plugin system: Rust trait-based plugins or external tool orchestration

### Crate Rename Audit

**Clear names (no change):**
- `normalize-core`, `normalize-derive`, `normalize-grammars` - foundational
- `normalize-languages` - Language trait implementations
- `normalize-typegen`, `normalize-openapi` - code generators
- `normalize-surface-syntax` - syntax translation
- `normalize-tools` - external tool interface
- `normalize-cli-parser` - CLI help output parsing

**Renames for clarity:**
- [n/a] `normalize-chat-sessions` — name is fine ("chat sessions" is specific enough)
- [n/a] `normalize-syntax-rules` — name is fine ("syntax rules" is specific enough)

**Structural split:**
- [x] `normalize-ecosystems` → split into two crates:
  - `normalize-ecosystems` - Ecosystem trait: project dependency management (cargo, npm, pip)
  - `normalize-package-index` - PackageIndex trait: distro/registry index ingestion (apt, brew, etc.)
  - Each has its own cache.rs with domain-specific caching
- Edit routing: workflow engine with LLM decision points
- Session/checkpoint: workflow state persistence
- PR/diff analysis: `normalize analyze --pr` or similar

## Backlog

### Lint / Analysis Architecture

See `docs/lint-architecture.md` for full design discussion.

**Current state:**
- `syntax-rules`: tree-sitter AST pattern matching (scm queries), single-file
- normalize-languages: extracts symbols, imports, calls, complexity for ~98 languages

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

**High priority:**
- [x] Language capabilities via `normalize-language-meta` crate (replaces hardcoded `is_programming_language` list)
  - `Capabilities` struct: `imports`, `callable_symbols`, `complexity`, `executable`
  - Per-language classification: data formats, markup, query, build DSL, shell, programming
  - Extensible for future metadata: type system, paradigm, syntax family
- [x] Module→file resolution for Rust (crate::, super::, self::) - ~10% of imports resolve to local files
  - Remaining unresolved: external crates (std::, serde::, etc.) - expected behavior
  - Future: trace re-exports from lib.rs/mod.rs for higher resolution rate

**Backlog - Deep Analysis (CodeQL-style):**
- [ ] Type extraction for top languages (TS, Python, Rust, Go)
- [ ] Data flow analysis
- [ ] Taint tracking
- Note: significant per-language effort, but tractable with LLM assistance

**Architectural analysis (near-term focus):**

Philosophy: **insights by default**, no configuration needed. Rules are for enforcement.

`normalize analyze architecture` complete:
- [x] Circular dependencies (DFS-based cycle detection)
- [x] Cross-imports (A↔B bidirectional coupling detection)
- [x] Coupling metrics: fan-in, fan-out, instability per module
- [x] Module→file resolution via `LocalDeps::resolve_local_import()` for Rust
- [x] Orphan modules (files with symbols never imported)
- [x] Symbol hotspots (most-called functions, filters generic methods)
- [x] Hub modules (high fan-in AND high fan-out - bottleneck detection)
- [x] Deep import chains (longest dependency paths, DFS with memoization)
- [x] Layer dependencies (inter-directory import flows, no config needed)

Next iteration:
- [ ] Boundary violation rules (configurable: "services/ cannot import cli/")
- [ ] Re-export tracing (follow `pub use` to resolve more imports)

Rules (custom enforcement, future):
- [ ] Module boundary rules ("services/ cannot import cli/")
- [ ] Threshold rules ("fan-out > 20 is error")
- [ ] Dependency path queries ("what's between A and B?")

**Rule tags system** (see `docs/lint-architecture.md`):
- [x] Built-in tags in `.scm`/`.dl` frontmatter (`tags = ["debug-print"]`)
- [x] `[rule-tags]` in `normalize.toml` — user-defined tag groups, tags can reference other tags
- [x] Per-rule `tags = []` in `normalize.toml` — additive, appends to built-in tags
- [x] Union semantics: same tag name = same concept, user tags extend built-in tags
- [x] `rules list --tag <tag> --enabled --disabled` (filters compose)
- [x] `rules run --tag <tag>`
- [x] `rules tags` subcommand — list all tags with origin (builtin/user), `--show-rules` to expand
- [x] `rules show <id>` — render full rule documentation offline
- [x] `rules enable`/`rules disable <tag-or-id>` — enable/disable by concept, with `--dry-run`
- [ ] Deterministic tag color hashing in `--pretty` output (curated palette, red/yellow reserved for severity)
- [x] Multi-paragraph rule doc block format: frontmatter → markdown comments → query (see `docs/lint-architecture.md`)

**Facts & Rules Architecture:**

Naming decision: "facts" over "index" because:
- normalize isn't limited to programming languages - facts are domain-agnostic
- Aligns with Datalog paradigm (facts + rules = analysis)
- "index" is vague; "facts" describes what we extract (assertions about code/data)

Plugin architecture: all rules (builtin and user) compile to dylibs via `abi_stable`:
- Same infrastructure for core team and users
- Builtins update independently of engine
- Users can share rule packs
- No special-casing between builtin vs user rules

Crate structure:
```
normalize-facts-core               # data types only (SymbolKind, Symbol, Import, FlatSymbol, etc.)
normalize-facts                    # full library: extraction + storage + queries (depends on core)
├── normalize-facts-rules-api      # stable ABI for rule plugins (abi_stable)
└── normalize-facts-rules-builtins # default rules (cycles, coupling, orphans, etc.)
```

Implementation:
- [x] Create `normalize-facts-core` with core data types (SymbolKind, Symbol, Import, Export, FlatSymbol, FlatImport, IndexedFile)
- [x] Update `normalize-languages` to re-export types from `normalize-facts-core`
- [x] Update `normalize` CLI to use types from `normalize-facts-core`
- [x] Create `normalize-facts` crate with extraction logic (Extractor, parsers)
- [x] Move ExtractResult filter methods (filter_types, filter_tests) to normalize-facts
- [x] Move FileIndex and SymbolParser from normalize to normalize-facts (storage layer)
- [x] Rename command `normalize index` → `normalize facts`
- [x] Add Ascent dependency to `normalize-facts-rules-api`
- [x] Define stable ABI with `abi_stable` (RulePack trait, Relations struct, Diagnostic output)
- [x] Map facts to Ascent relations: `symbol(file, name, kind)`, `import(from, to)`, `call(caller, callee)`
- [x] Rewrite one builtin rule in Datalog (circular dependencies) as proof of concept
- [x] Dylib loading: find/load rule packs from known paths
- [x] `normalize facts check <rules.dl>` - interpreted Datalog via ascent-interpreter
- [ ] `normalize facts compile <rules.dl>` command to build custom packs (sandboxed codegen)
- [ ] Self-install builtin dylib: `normalize facts rules` should auto-install compiled builtins to `~/.local/share/normalize/rules/` on first run (or at build/install time). Currently requires manual copy.
- [x] More builtin interpreted fact rules: unused_import, missing_export, deep_nesting, layering_violation, barrel_file, bidirectional_deps (now 17 builtins, 3 enabled by default)
- [x] Unify `normalize rules` namespace: `normalize rules list/run/add` now handle both syntax + fact rules. `facts check` delegates to unified infrastructure.

**`implements` relation extraction — completed for 19 languages:**

- [x] TypeScript, JavaScript (shared ecmascript module)
- [x] Python, Java, C++, Scala, Ruby, Dart, D (Tier 1)
- [x] C#, Kotlin, Swift, PHP, Objective-C, MATLAB (Tier 2)
- [x] GraphQL, Haskell (Tier 3)
- [x] Rust (trait impls merged into struct via `merge_rust_impl_blocks`)

Skipped (grammar limitations or semantic mismatch):
- HLSL — no class container kind (only structs)
- VB, F# — tree-sitter grammars parse `Inherits`/`Implements`/`inherit` as ERROR nodes
- OCaml — `class_definition` not in container_kinds

Also fixed `node_name` bugs in Kotlin, Objective-C, and GraphQL that prevented class extraction.

### Language Capability Traits

See `docs/language-capability-traits.md` for full design.

The monolithic `Language` trait couples two growth axes: adding a language requires implementing all methods, adding a feature requires sweeping all 98 impls. Split into optional capability traits, following the `LocalDeps` precedent.

Trigger: split a capability when >50% of languages would return stubs. `has_symbols()` is the existing smell.

- [ ] `LanguageEmbedded` — extract `embedded_content()`, already past sparsity threshold (only Vue, HTML, ~3 others)
- [ ] Add `as_symbols()`, `as_imports()`, `as_complexity()`, `as_edit()` query methods to `Language` with `None` defaults (Option B from design doc — incremental, no flag-day)
- [ ] Migrate call sites to use capability queries where "not supported" differs from "empty"
- [ ] Remove `has_symbols()` once capability queries cover all its uses

### normalize-typegen

**Infrastructure (DONE):**
- [x] `Backend` trait with registry (hybrid pattern for user-defined backends)
- [x] Feature flags: `backend-*` prefix (e.g., `backend-typescript`, `backend-rust`, `backend-zod`)
- [x] All 7 backends implement trait: TypeScript, Zod, Valibot, Python, Pydantic, Go, Rust
- [x] Snapshot tests for all backends (20 snapshots)

**Input Parsers:**
- [x] JSON Schema parser (`parse_json_schema`)
- [x] OpenAPI parser (`parse_openapi`)
- [ ] Protobuf parser - read .proto files to IR
- [ ] GraphQL schema parser - read GraphQL SDL to IR
- [x] TypeScript type parser - extract type definitions from .ts files

**Output Backends:**
- [ ] JSON Schema output - emit IR back to JSON Schema (for validation/documentation)
- [ ] GraphQL SDL output - emit IR as GraphQL types
- [ ] Protobuf output - emit IR as .proto definitions

**CLI Enhancements:**
- [x] Support stdin input (`normalize generate types -`)
- [ ] Multiple output files (`--split` to emit one file per type)
- [ ] Dry-run mode (`--dry-run` to preview without writing)

**IR Improvements:**
- [ ] Validation: ensure IR is well-formed before generating (no circular refs, valid names)
- [ ] Nullable vs Optional distinction (some languages care)
- [ ] Default values support in Field
- [ ] Constraints (min/max, pattern, format) for validation libraries

### normalize-surface-syntax

**Infrastructure (DONE):**
- [x] `Reader` and `Writer` traits with registry
- [x] Feature flags: `read-*` and `write-*` prefixes
- [x] CLI: `normalize translate` command

**Readers:**
- [x] TypeScript reader (tree-sitter based)
  - [x] Variables (const, let), binary expressions, function calls
  - [x] If/else, while, for loops
  - [x] Functions (declarations, arrow functions)
  - [x] Arrays, objects
  - [ ] Classes, interfaces, type annotations
  - [x] Try/catch/finally
  - [x] Switch statements (→ nested if/else in IR)
  - [ ] Spread operator, destructuring
  - [ ] Template literals
  - [ ] Async/await
- [x] Lua reader (tree-sitter based)
  - [x] Variables (local), binary expressions, function calls
  - [x] If/elseif/else, while, for loops (numeric, generic)
  - [x] Functions (declarations, anonymous)
  - [x] Tables (array-like, record-like)
  - [ ] Metatables, metamethods
  - [x] Varargs (`...`) - params and expression context
  - [x] Repeat-until loops (→ while(true) + break)
  - [x] Multiple return values (→ array expression)
  - [ ] String methods (`:method()` syntax)
- [x] Python reader (tree-sitter based)
- [ ] JavaScript reader (or reuse TypeScript reader with flag?)

**Writers:**
- [x] Lua writer
  - [x] Variables, binary ops, function calls
  - [x] Control flow (if, while, for)
  - [x] Functions, tables
  - [ ] Verify idiomatic output (use `and`/`or` vs `&&`/`||`)
  - [ ] String escaping edge cases
- [x] TypeScript writer
  - [x] Variables (const, let), binary ops, function calls
  - [x] Control flow (if, while, for)
  - [x] Arrow functions, objects/arrays
  - [ ] Type annotations (when available in IR)
  - [ ] Verify semicolon placement
  - [ ] Template literal output for complex strings
- [x] Python writer
- [ ] JavaScript writer (or reuse TypeScript writer?)

**Testing:**
- [x] Basic roundtrip tests in registry.rs
- [x] Roundtrip tests with `structure_eq`: TS → IR₁ → Lua → IR₂ → assert `ir1.structure_eq(&ir2)`
  - IR is "reasonable middle ground", not strict LCD
  - Core fields: must match (names, expressions, control flow structure)
  - Hint fields: normalized in comparison (`mutable`, `computed` on string-literal member access)
  - `structure_eq(&self, &other) -> bool`: in-place comparison, no cloning
- [x] Snapshot tests for reader outputs (verify parsed IR is correct)
- [x] Snapshot tests for writer outputs (verify emitted code is correct)
- [ ] Edge case tests: nested expressions, complex control flow, Unicode strings

**IR Improvements:**
- [ ] Comments preservation (for documentation translation)
- [ ] Source locations (for error messages, debugging)
- [ ] Import/export statements
- [ ] Class definitions, method definitions
- [ ] Type annotations (optional, for typed languages)
- [ ] Pattern matching / destructuring
- [x] Exception handling (try/catch/finally)

### Feature flags for customizability
Add feature flags to crates so consumers can opt out of implementations they don't need.
Use consistent prefixes within each crate:
- [x] normalize-languages: `lang-*` (e.g., `lang-typescript`, `lang-rust`) and groups `langs-*` (e.g., `langs-core`, `langs-functional`)
- [x] normalize-ecosystems: `ecosystem-*` (e.g., `ecosystem-npm`, `ecosystem-cargo`, `ecosystem-python`)
- [x] normalize-chat-sessions: `format-*` (e.g., `format-claude`, `format-codex`, `format-gemini`, `format-normalize`)
- [x] normalize-tools: `tool-*` individual (e.g., `tool-ruff`, `tool-clippy`) + `tools-*` language groups (e.g., `tools-python`, `tools-rust`)

### Workflow Engine
- [x] Streaming output for `auto{}` driver
- JSON Schema for complex action parameters (currently string-only)
- Workflow chaining: automatically trigger next workflow based on outcome (e.g., Investigation → Fix → Review)

### Workflow Documentation (see `docs/workflows/`)
Document edge-case workflows - unusual scenarios that don't fit standard patterns:

**Investigation:**
- [x] Reverse engineering code - undocumented/legacy code with no context
- [x] Reverse engineering binary formats - file formats, protocols without docs
- [x] Debugging production issues - logs/traces without local reproduction
- [x] Performance regression hunting - finding what made things slow
- [x] Flaky test debugging - non-deterministic failures, timing issues

**Modification:**
- [x] Merge conflict resolution - understanding both sides, correct resolution
- [x] Cross-language migration - porting code (Python→Rust, JS→TS)
- [x] Breaking API changes - upstream dependency changes that break your code
- [x] Dead code elimination - safely removing unused code paths

**Synthesis:**
- [x] High-quality code synthesis - D×C verification for low-data domains
- [x] Binding generation - FFI/bindings for libraries
- [x] Grammar/parser generation - parsers from examples + informal specs

**Meta:**
- [x] Onboarding to unfamiliar codebase - systematic exploration
- [x] Documentation synthesis - generating docs from code
- [x] Cross-workflow analysis - extract shared insights, patterns, principles after all workflows documented

**Security/Forensic:**
- [x] Cryptanalysis - analyzing crypto implementations
- [x] Steganography detection - finding hidden data
- [x] Malware analysis - understanding malicious code (read-only)

**Example codebases for workflow testing:**
- viwo: DSL/framework/scripting language with insufficient testing, numerous bugs
  - Good for: debugging legacy code, reverse engineering code workflows
  - Details available on request when tackling this

**Research (completed):**
- [x] https://github.com/ChrisWiles/claude-code-showcase - Claude Code configuration patterns
  - Skills: markdown docs with frontmatter, auto-triggered by scoring (keywords 2pts, regex 3, paths 4, directory 5, intent 4)
  - Agents: specialized assistants with severity levels (Critical/Warning/Suggestion)
  - Hooks: PreToolUse, PostToolUse, UserPromptSubmit, Stop lifecycle events
  - GitHub Actions: scheduled maintenance (weekly quality, monthly docs sync, dependency audit)
  - **Actionable for normalize:**
    - Script/workflow selection scoring (match prompts to relevant `.normalize/scripts/`)
    - Formalize auditor severity levels in output format
    - Expand hook triggering beyond current implementation
    - CI integration patterns for automated quality checks

### Package Management
- `normalize package install/uninstall`: proxy to ecosystem tools (cargo add, npm install, etc.)
  - Very low priority - needs concrete use case showing value beyond direct tool usage
  - Possible value-adds: install across all ecosystems, auto-audit after install, config-driven installs

### Package Index Fetchers (normalize-ecosystems)

**Full coverage tracking**: See `docs/repository-coverage.md` for complete repository list.

**API Verification Results**:

✅ WORKING:
- apk: Alpine - APKINDEX.tar.gz parsing (multi-member gzip + tar)
- artix: packages.artixlinux.org/packages/search/json/?name={name} (Arch-compatible format)
- conan: conan.io/api/search JSON API
- dnf: mdapi.fedoraproject.org/rawhide/pkg/{name} (JSON)
- freebsd: pkg.freebsd.org packagesite.pkg (zstd tar + JSON-lines)
- gentoo: packages.gentoo.org/packages/{cat}/{name}.json (JSON)
- guix: guix.gnu.org/packages.json (gzip-compressed JSON array, ~30k packages)
- nix: search.nixos.org Elasticsearch (requires POST with query JSON)
- opensuse: download.opensuse.org repodata/primary.xml.zst (zstd XML)
- pacman/aur: aur.archlinux.org/packages-meta-ext-v1.json.gz (full archive)
- void: repo-default.voidlinux.org x86_64-repodata (zstd tar + XML plist)

⚠️ XML ONLY (needs XML parsing):
- choco: community.chocolatey.org/api/v2 returns NuGet v2 OData/Atom XML

❌ NO PUBLIC API (removed from fetchers):
- openbsd: openports.pl - HTML only - removed
- netbsd: pkgsrc.se - HTML only - removed
- swiftpm: Swift Package Index requires authentication for API access
- stackage: No JSON API (endpoints redirect, snapshot URLs 404)
- ghcr: GitHub Container Registry requires authentication (401)
- gradle: Plugin portal API returning 404 (plugins.gradle.org/api/plugins)

**Implemented fetchers** (57 total: 17 distro, 4 Windows, 3 macOS, 2 cross-platform, 1 container, 2 mobile, 28 language):
- [x] APK (Alpine): APKINDEX.tar.gz with checksums, deps, archive URLs
- [x] Artix Linux: Arch-based, shares arch_common logic with pacman
- [x] NixOS/Nix: search.nixos.org Elasticsearch API
- [x] Void Linux: zstd tar + XML plist parsing
- [x] Gentoo: packages.gentoo.org API
- [x] Guix: packages.guix.gnu.org with fetch_all support
- [x] Slackware: SlackBuilds.org via GitHub raw .info files
- [x] FreeBSD: zstd tar + JSON-lines parsing (packagesite.pkg)
- [x] openSUSE: zstd XML parsing (repodata/primary.xml.zst)
- [x] CachyOS: Arch-based, uses arch_common
- [x] EndeavourOS: Arch-based, uses arch_common
- [x] Manjaro: repo.manjaro.org database parsing + AUR
- [x] Copr: Fedora community builds (copr.fedorainfracloud.org API)
- [x] Chaotic-AUR: chaotic-backend.garudalinux.org JSON API
- [x] MSYS2: packages.msys2.org API (Windows development)
- [x] MacPorts: ports.macports.org API
- [x] Snap: api.snapcraft.io (requires Snap-Device-Series header)
- [x] DUB: code.dlang.org API (D packages)
- [x] Clojars: clojars.org API (Clojure packages)
- [x] CTAN: ctan.org JSON API (TeX/LaTeX packages)
- [x] Racket: pkgs.racket-lang.org (Racket packages)
- [x] Bioconductor: bioconductor.r-universe.dev API (R bioinformatics)
- [x] Hunter: GitHub cmake parsing (C++ packages)
- [x] Docker: hub.docker.com API (container images)
- [x] F-Droid: f-droid.org API (Android FOSS apps)
- [x] vcpkg: GitHub baseline.json + port manifests (C++ packages)
- [x] Termux: GitHub build.sh parsing (Android terminal packages)
- [x] Conan: conan.io/api/search JSON API

**Note**: Debian-derivatives (Ubuntu, Mint, elementary) use apt fetcher.
Arch-derivatives (Manjaro, etc.) can use pacman fetcher.

**fetch_all implementations**:
- [x] APK: APKINDEX.tar.gz (main + community repos)
- [x] AUR: packages-meta-ext-v1.json.gz (~30MB, ~5min refresh)
- [x] Homebrew: formula.json
- [x] Deno: paginated API
- [x] Guix: packages.json
- [x] Arch official: .db.tar.zst databases (gzip + zstd decompression)
- [x] RubyGems: Compact Index /versions endpoint (streaming)
- [x] NuGet: Catalog API (incremental updates)
- Crates.io: has db-dump.tar.gz (~800MB - could implement, low priority)
- npm: has registry replicate API (massive - probably not practical)
- PyPI: no bulk API (BigQuery only alternative)
- Docker Hub: no bulk index (millions of images, search API paginated/rate-limited, no `/v2/_catalog`)

**fetch_all design**: Should return one PackageMeta per *version*, not per package:
- Distro indexes already do this (different repos have different versions)
- When implementing for npm/crates.io, expand to all versions
- Keep `fetch()` returning single latest for quick lookups
- [x] `fetch_all_versions(name)` added to trait - returns Vec<PackageMeta> for all versions
  - npm: full implementation with per-version deps, engines, checksums
  - cargo: full implementation with per-version features, MSRV, checksums
  - docker: full implementation with per-tag digest, size, architectures, OS
  - Default: falls back to fetch_versions() with minimal data

**Struct completeness audit**: Each fetcher should populate all available fields from their APIs:
- keywords, maintainers, published dates where available
- downloads counts from APIs that provide them
- archive_url and checksum for verification
- extra field for ecosystem-specific metadata not in normalized fields

**Performance improvements needed**:
- [x] Streaming/iterator API: Added `iter_all()` to PackageIndex trait with lazy iteration. APT implements streaming `AptPackageIter` that parses line-by-line.
- [x] Parallel repo fetching: openSUSE fetches repos in parallel with rayon (~4x speedup)

**Multi-repo coverage done**:
- [x] openSUSE: 36 repos (Tumbleweed, Leap 16.0, Leap 15.6 × OSS/Non-OSS/Updates, Factory, source RPMs, debug symbols, community repos: Games, KDE, GNOME, Xfce, Mozilla, Science, Wine, Server)
- [x] Arch Linux: 12 repos (core, extra, multilib, testing, staging, gnome/kde-unstable, AUR)
- [x] Artix Linux: 15 repos (system, world, galaxy, lib32, asteroids × stable/gremlins/goblins)
- [x] Alpine/APK: 11 repos (edge, v3.21, v3.20, v3.19, v3.18 × main/community/testing)
- [x] FreeBSD: 5 repos (FreeBSD 13/14/15 × quarterly/latest)
- [x] Void Linux: 8 repos (x86_64/aarch64 × glibc/musl × free/nonfree)

**Multi-repo coverage done**:
- [x] Manjaro: 10 repos (stable/testing/unstable × core/extra/multilib + AUR)
- [x] Debian/APT: 21 repos (stable/testing/unstable/experimental/oldstable × main/contrib/non-free + backports)
- [x] Fedora/DNF: 6 repos (Fedora 39/40/41, Rawhide, EPEL 8/9)
- [x] Ubuntu: 22 repos (Noble 24.04/Jammy 22.04/Oracular 24.10 × main/restricted/universe/multiverse + updates/security/backports)
- [x] Nix: 5 channels (nixos-stable, nixos-unstable, nixpkgs-unstable, nixos-24.05, nixos-24.11)
- [x] CachyOS: 8 repos (cachyos, cachyos-v3/v4, core-v3/v4, extra-v3/v4, testing)
- [x] EndeavourOS: 5 repos (endeavouros, core, extra, multilib, testing)
- [x] Gentoo: 5 repos (gentoo, guru, science, haskell, games overlays)
- [x] Guix: 2 channels (guix, nonguix)
- [x] Slackware: 3 versions (current, 15.0, 14.2)
- [x] Scoop: 8 buckets (main, extras, versions, games, nerd-fonts, java, php, nonportable)
- [x] Chocolatey: community repository
- [x] WinGet: 2 sources (winget, msstore)
- [x] Flatpak: 2 remotes (flathub, flathub-beta)
- [x] Snap: 4 channels (stable, candidate, beta, edge)
- [x] Conda: 4 channels (conda-forge, defaults, bioconda, pytorch)
- [x] Maven: 3 repos (central, google, sonatype)
- [x] Docker: 4 registries (docker-hub, ghcr, quay, gcr)

**Multi-repo coverage remaining**:

All major package managers now have multi-repo support. Remaining unit-struct fetchers are single-source registries where multi-repo doesn't apply (npm, PyPI, crates.io, etc.).

### Complexity Hotspots (reduced - max now 58)
- [x] `handle_glob_edit` (76→41): extracted insert_at_destination, position_op_name
- [x] `cmd_view_file` (69→48): extracted format_skeleton_lines, print_fisheye_imports
- [x] `cmd_edit` (67→51): extracted insert_single_at_destination
- [x] `cmd_daemon` (66→54): extracted handle_response
- [x] `cmd_view_symbol` (65→47): extracted print_smart_imports
- [x] `categorize_command` (64→26): extracted categorize_cargo, categorize_npm_run, categorize_js_runner
- [x] `analyze/mod.rs:run` (63→51): extracted resolve_diff_and_filter
- [x] `analyze_architecture` (62→split): extracted build_import_graph, compute_coupling_and_hubs, detect_cross_imports, find_orphan_modules, find_symbol_hotspots
- [x] `is_structural_line` (60→14): extracted per-language is_rust/js/python/go/generic_structural
- [ ] `crates/normalize/src/commands/analyze/query.rs:cmd_query` (58)
- [ ] `crates/normalize/src/commands/daemon.rs:cmd_daemon` (54)
- [ ] `crates/normalize-syntax-rules/src/runner.rs:evaluate_predicates` (53)
- [ ] `crates/normalize/src/commands/analyze/mod.rs:run` (51)
- [ ] `crates/normalize/src/commands/tools/lint.rs:cmd_lint_run` (48)
- [ ] `crates/normalize/src/tree.rs:collect_highlight_spans` (46)

### Package Index Backlog (simplest → complex)

**1. Struct completeness audit** (DONE)
- [x] Audited 10 high-value fetchers: cargo, npm, pip, gem, go, hex, hackage, pub_dev, composer, choco
- [x] Added keywords, maintainers, published, downloads, archive_url, checksum where APIs provide them
- [x] Remaining fetchers use ..Default::default() - lower priority

**2. Chocolatey XML parsing** (DONE)
- [x] quick-xml already in deps, full OData/Atom XML parsing implemented
- [x] Extracts full metadata from XML

**3. RubyGems fetch_all** (DONE)
- [x] Implemented Compact Index /versions endpoint with streaming GemVersionsIter
- [x] Deduplicates gems (versions file is append-only)

**4. NuGet catalog API** (DONE)
- [x] Implemented catalog/index.json traversal with NuGetCatalogIter
- [x] Supports incremental updates (pages loaded on demand)

**5. Arch official fetch_all** (DONE)
- [x] Parse .db.tar.zst package databases (added zstd decompression)
- [x] Handles desc files within archives

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
- [x] `--allow` for duplicate-functions: accept line range like output suggests (e.g., `--allow src/foo.rs:10-20`)
- Unnecessary aliases: `let x = Foo; x.bar()` → `Foo.bar()`. Lint for pointless intermediate bindings.
- [x] Chained if-let: edition 2024 allows `if let Ok(x) = foo() && let Some(y) = bar(x)`. Audit complete.
- PR/diff analysis: `normalize analyze --pr` or `--diff` for changed code focus (needs broader analysis workflow design)
- [x] Test gap analysis: `normalize analyze test-gaps` - find public functions with no direct test caller. See `docs/design/test-gaps.md`
- [x] Validate node kinds against grammars: `validate_unused_kinds_audit()` in 99 language files, runs as test
- [x] Directory context: `normalize context`, `view --dir-context`
- Deduplicate SQL queries in normalize: many ad-hoc queries could use shared prepared statements or query builders (needs design: queries use different execution contexts - Connection vs Transaction)
- Detect reinvented wheels: hand-rolled JSON/escaping when serde exists, manual string building for structured formats, reimplemented stdlib. Heuristics unclear. Full codebase scan impractical. Maybe: (1) trigger on new code matching suspicious patterns, (2) index function signatures and flag known anti-patterns, (3) check unused crate features vs hand-rolled equivalents. Research problem.
- [x] `analyze duplicate-blocks`: subtree-level clone detection with containment suppression. See `docs/design/duplicate-detection.md`
- [x] Fuzzy/partial clone detection (`analyze similar-blocks`): MinHash LSH over AST token shingles, 128-dim signatures, 32 bands. Containment + overlap suppression. See `docs/design/duplicate-detection.md`.
- [x] Skeleton mode (`--skeleton` on `similar-blocks`): serialize structural skeleton only — keep control flow nodes, replace bodies with `<body>` placeholder. Relaxed size ratio (0.2 vs 0.5), degenerate skeleton filtering (min token count + 30% unique token diversity). See `docs/design/duplicate-detection.md`.
- [x] `--skip-functions` on `duplicate-blocks`: skip function/method nodes to avoid overlap with `duplicate-functions`.
- [x] `similar-functions`: MinHash LSH scoped to function nodes; named-symbol output; `--skeleton` support.
- [x] Allow files for `duplicate-blocks` (`duplicate-blocks-allow`) and `similar-blocks` (`similar-blocks-allow`). Key format: `file:func:start-end` or `file:start-end`. `--allow <location> --reason <text>` flags on both commands.
- [x] Improve duplicate/clone detection to work usefully out of the box:
  - [x] `duplicate-functions`: same-name groups suppressed by default (`--include-trait-impls` to restore); 351→185 groups
  - [x] `similar-functions`: same-name pairs suppressed by default; min-lines 5→10, similarity 0.80→0.85; 3781→537 pairs
  - [x] `similar-blocks`: containing-function same-name suppression (`--include-trait-impls`); min-lines 5→10, similarity 0.80→0.85; 4489→1103 pairs → 600 after suppression
  - [x] `duplicate-types`: IDF-weighted Jaccard (rare fields outweigh `name`/`file`/`line`), require ≥3 common fields; 154→16 pairs
  - [x] All analyze commands: auto-exclude lockfiles from `is_source_file`
  - [x] `.normalize/config.toml`: permanent `exclude = ["crates/normalize-languages/src"]` for this codebase — ~98 language files implement the same trait with identical Symbol constructors across different method names (`extract_function`, `extract_container`, `extract_type`); not suppressible by same-name heuristic since they cross method boundaries
  - Remaining improvements:
    - Per-subcommand excludes in config: `[analyze.similar-blocks] exclude = [...]` so language-file exclusion doesn't affect `analyze rules`, `analyze complexity`, etc. (currently the global `[analyze] exclude` is too coarse)
    - "Parallel impl directory" heuristic: if >N pairs originate from the same directory pair, fold them into a single suppressed note (e.g., "48 pairs suppressed within normalize-languages/ — likely parallel Language trait implementations")
    - `similar-blocks` / `similar-functions`: cross-file same-containing-function suppression covers same-method-name in different files; doesn't cover same-body-pattern across different method names (the Language impl case)
    - Consider min-lines bump for `similar-blocks` (currently 10) — the 19-line Symbol constructor is below many useful thresholds; maybe 15-20 default would further cut noise without missing real clones
- Syntax-based linting: see `docs/design/syntax-linting.md`
  - [x] Phase 1: `normalize analyze ast`, `normalize analyze query` (authoring tools)
  - [x] Phase 1b: `normalize analyze rules` reads .normalize/rules/*.scm with TOML frontmatter
  - [x] Phase 3a: builtin rules infrastructure (embedded + override + disable)
  - [x] Phase 2: severity config override, SARIF output
  - Phase 3b: more builtin rules, sharing, auto-fix (see `docs/design/builtin-rules.md`)
    - [x] Extended language coverage: Python (print-debug, breakpoint), Go (fmt-print), Ruby (binding-pry)
    - [x] Rule sharing/import mechanism (`normalize rules add/update/list/remove`)
    - [x] Auto-fix support (`normalize analyze rules --fix`)
  - [x] Project manifest parsing: extract version/config from project manifests
    - RustSource: Cargo.toml (edition, resolver, name, version)
    - TypeScriptSource: tsconfig.json + package.json (target, module, strict, node_version)
    - PythonSource: pyproject.toml (requires_python, name, version)
    - GoSource: go.mod (version, module)
    - Each source finds nearest manifest for the file being analyzed
  - [x] Rule conditionals: `requires` predicates beyond path-based `allow`
    - Pluggable RuleSource trait for data sources
    - Built-in sources: env, path, git, rust, typescript, python, go
    - Operators: exact match, >=, <=, !
    - Example: `requires = { "rust.edition" = ">=2024" }` for chained if-let
    - Example: `requires = { "env.CI" = "true" }` for stricter CI-only rules
  - Semantic rules system: for rules needing cross-file analysis (import cycles, unused exports, type mismatches). Current syntax-based rules are single-file AST queries; semantic rules need index-backed analysis. Separate infrastructure, triggered differently (post-index vs per-file).
  - [x] Phase 4: combined query optimization (single-traversal multi-rule matching)
    - Achieved via tree-sitter combined queries (simpler than full tree automata)
    - Performance: 4.3s → 0.75s (5.7x faster) for 13 rules, ~550 findings
    - Implementation: concatenate all rule queries per grammar, use pattern_index to map matches
    - Key insight: predicates scope per-pattern even with shared capture names

### Script System
- TOML workflow format: structured definition (steps, actions) - **deferred until use cases are clearer**
  - Builtin `workflow` runner script interprets TOML files
  - Users can also write pure Lua scripts directly
- Lua test framework: test discovery for `.normalize/tests/` (test + test.property modules done)
  - Command naming: must clearly indicate "normalize Lua scripts" not general testing (avoid `@test`, `@spec`, `@check`)
  - Alternative: no special command, just run test files directly via `normalize <file>`
- [x] Agent module refactoring: extracted 6 submodules (parser, session, context, risk, commands, roles)
  - agent.lua reduced from ~2300 to ~1240 lines (46% reduction)
  - Remaining: run_state_machine (~400 lines), M.run (~650 lines) - core agent logic, self-contained
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
- [x] Symbol history: `normalize view path/Symbol --history [N]`
  - Shows last N changes to a symbol via git log -L (default: 5)
  - Works for both symbols and files
- Documentation freshness: tooling to keep docs in sync with code
  - For normalize itself: keep docs/cli/*.md in sync with CLI behavior (lint? generate from --help?)
  - For user projects: detect stale docs in fresh projects (full normalize assistance) and legacy codebases (missing/outdated docs)
  - Consider boy scout rule: when touching code, improve nearby docs
- [x] Case-insensitive matching (`-i` flag): `text-search` ✓, `view` ✓, `edit` ✓ all have it
- `normalize fetch`: web content retrieval for LLM context (needs design: chunking, streaming, headless browser?)
- [x] Multi-file batch edit: `normalize edit --batch edits.json` (see docs/design/batch-edit.md)
- Semantic refactoring: `normalize edit <glob> --before 'fn extract_attributes' 'fn extract_attributes(...) { ... }'`
  - Insert method before/after another method across multiple files
  - Uses tree-sitter for semantic targeting (not regex)
  - `--batch` flag for multiple targets in one invocation
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

### Agent Future (deferred complex features)

**Test selection** - run only tests affected by changes
- Prerequisite: Call graph extraction in indexer (who calls what)
- Prerequisite: Test file detection (identify test functions/modules)
- Map modified functions → tests that call them
- Integration with test runners (cargo test, pytest, jest)

**Task decomposition** - break large tasks into validated subtasks
- Prerequisite: Better planning prompts (current --plan is basic)
- Prerequisite: Subtask validation (each step must pass before next)
- Agent creates plan with discrete steps
- Each step is a mini-agent session with its own validation
- Rollback entire task if any step fails

**Cross-file refactoring** - rename/move symbols across codebase
- Prerequisite: Symbol graph in indexer (callers, callees, types)
- Prerequisite: Import/export tracking per language
- Find all usages via `normalize analyze --callers Symbol`
- Edit each usage atomically (all-or-nothing)
- Update imports/exports as needed

**Human-in-the-loop escalation** - ask user when stuck
- Prerequisite: Interactive mode in agent (currently non-blocking)
- Prerequisite: Stuck detection (beyond loop detection)
- When agent can't proceed, pause and ask user
- User provides guidance, agent continues
- Graceful degradation when non-interactive

**Partial success handling** - apply working edits, report failures
- Trade-off: Conflicts with atomic editing (all-or-nothing is often safer)
- Use case: Large batch where some files have issues
- Report which succeeded, which failed, why
- Consider: Is this actually desirable? Atomic may be better.

**Agent refactoring** - COMPLETE:
- Split into 6 modules: parser, session, context, risk, commands, roles
- Removed v1 freeform loop, kept only state machine
- agent.lua: 2300 → 762 lines (67% reduction)

### Agent Testing

**Observations** (74 sessions analyzed):
- Success rates: Anthropic 58%, Gemini 44%
- Auditor role completes in 2-4 turns for focused tasks
- Investigator can loop on complex questions (mitigated by cycle detection)
- --diff flag works well for PR-focused analysis
- Session logs: `.normalize/agent/logs/*.jsonl`

**Ongoing**:
- Document friction points: where does the agent get stuck?
- Prompt tuning based on observed behavior

**Known Gemini issues** (still present):
- Hallucinates command outputs (answers before seeing results)
- Random Chinese characters mid-response
- Intermittent 500 errors and timeouts
- Occasionally outputs duplicate/excessive commands
- SSL certificate validation failures in some environments (`InvalidCertificate(UnknownIssuer)` - missing CA certs or SSL inspection proxy)
- **Google blocks Claude Code cloud environments**: 403 Forbidden on all Gemini API requests from Claude Code cloud infrastructure (even with valid API key and SSL bypass)

**OpenRouter in cloud environments**:
- SSL bypass works (connects to OpenRouter successfully)
- Gemini models via OpenRouter: 503 with upstream SSL error (unclear root cause, likely environment-specific)
- Claude models via OpenRouter: JSON parsing error (API response format mismatch with rig)
- Not worth debugging further in this environment - likely network/proxy/environment issues

**Roles implemented**:
- [x] Investigator (default): answers questions about the codebase
- [x] Auditor: finds issues (security, quality, patterns)
  - Usage: `normalize @agent --audit "find unwrap on user input"`
  - Structured output: `$(note SECURITY:HIGH file:line - description)`
  - Planner creates systematic audit strategy

**Prompt tuning observations**:
- Claude sometimes uses bash-style `view ...` instead of `$(view ...)`
- Evaluator occasionally outputs commands in backticks

### Agent Future

Core agency features complete (shadow editing, validation, risk gates, retry, auto-commit).

**Remaining**:
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

### Agent Observations

- **FOOTGUN: Claude Code cwd**: `cd` in Bash commands persists across calls. E.g., `cd foo && perl ...` breaks subsequent calls. Always use absolute paths.
- Claude works reliably with current prompt
- Context compaction unreliable in practice (Claude Code + Opus 4.5 lost in-progress work)
- Normalize's dynamic context reshaping avoids append-only accumulation problems
- LLM code consistency: see `docs/llm-code-consistency.md`
- Large file edits: agentic tools struggle with large deletions (Edit tool match failures)
- **View loops**: Claude can get stuck viewing same files repeatedly without extracting info (session 67xvhqzk: 7× `view commands/`, 7× `view mod.rs`, 15 turns, task incomplete)
  - Likely cause: `view` output doesn't contain the info needed (e.g., CLI command names in Rust enums/structs require deeper inspection)
  - Possible fixes: better prompting, richer view output, or guide agent to use text-search for specific patterns
  - Contrast: text-search task succeeded in 1 turn (session 6ruc3djn) - tool output contained answer directly
  - Pattern: agent succeeds when tool output = answer, struggles when output requires interpretation/assembly
- **Pre-answering**: [FIXED] See `docs/experiments/agent-prompts.md` for full analysis
  - Root cause: task framing made single-turn look like correct completion
  - Fix: "investigator" role + concrete example + evidence requirement
  - Results: 3/3 correct with new prompt, 2-8 turns, no pre-answering
  - Key insight: concrete example in prompt prevents LLM defaulting to XML function calls
- **Ephemeral context**: Verified working correctly
  - Turn N outputs → visible in Turn N+1 `[outputs]` → gone by Turn N+2 unless `$(keep)`
  - 1-turn window is intentional: LLM needs to see results before deciding what to keep
- **Context uniqueness hypothesis**: identical context between any two LLM calls = error/loop
  - Risk: same command twice → same outputs → similar contexts → loop potential
  - Mitigation: `is_looping()` catches repeated commands, not identical context from different commands
- **CRITICAL: Using grep patterns with text-search** - Claude Code used `\|` (grep OR syntax) with text-search
  - text-search was specifically renamed from grep to avoid regex escaping confusion
  - Agent failed to use tool correctly despite it being in the command list
  - This shows agents don't understand tool semantics, just syntax
  - Need better tool descriptions or examples in prompt
- **Evaluator exploring instead of concluding**: [FIXED] Session zj3y5yu4 - evaluator output commands in backticks instead of $(answer)
  - Root cause: passive prompt "Do NOT run commands" → models interpret as "describe what to run"
  - Fix: strong role framing ("You are an EVALUATOR"), banned phrases ("NEVER say 'I need to'"), good/bad examples
  - Results: 4 turns vs 12 turns (no answer) for same query
  - Key insight: role assertion + explicit prohibitions + concrete examples beats instruction-only prompts
- **Dogfooding session (2026-01-07)**:
  - Gemini 500 errors remain intermittent (hit on first task, next 3 succeeded)
  - Agent occasionally uses `$(run ls -R)` instead of `$(view .)` - prefers shell over normalize tools
  - Investigator: 4 turns for config structure query, correct answer, good line-range viewing
  - Auditor: 2 turns for unwrap() audit, parallel search commands, accurate file:line findings
  - Pattern: auditor role executes parallel searches efficiently (5 commands turn 1, synthesized turn 2)

### Code Quality / Consistency

**OutputFormatter migration** - Ensure all user-facing output uses the trait:
- [x] SessionAnalysis (sessions/analysis.rs)
- [x] DocCoverageReport (commands/analyze/docs.rs) - removed manual to_json()
- [x] FileLengthReport (commands/analyze/files.rs) - removed manual to_json()
- [x] SecurityReport (commands/analyze/report.rs) - auto Serialize
- [x] AnalyzeReport (commands/analyze/report.rs) - removed 133 lines of to_json()
- [x] ComplexityReport (analyze/complexity.rs) - removed 80+ lines of print functions
- [x] LengthReport (analyze/function_length.rs) - removed 80+ lines of print functions
- [ ] Remaining commands with manual format handling (~18 commands):
  - Hotspots, check_refs, duplicates, stale_docs, check_examples
  - View commands (symbol, tree, file, history, lines)
  - Sessions commands (stats, plans, list)
  - Daemon commands (11+ manual branches)
  - Edit commands (15+ manual branches)
  - Index, rules, history commands
- Benefits: ~475 lines of boilerplate removed so far, automatic --compact/--pretty/--json/--jq support

### Session Analysis

**normalize-chat-sessions refactor** - see `docs/design/sessions-refactor.md`
- [x] Split parsing from analysis: `LogFormat::parse()` → unified `Session` type
- [x] Move analysis to consumers (normalize CLI uses `parse()` + local `analyze_session()`)
- [x] Remove `analyze()` from LogFormat trait (analysis now in `crates/normalize/src/sessions/analysis.rs`)

**Recently Added (2026-01-24)**:
- [x] Tool patterns: common sequences across sessions (e.g., "Read → Edit" 42×)
  - Extracts subsequences length 2-5 from all tool chains
  - Shows top 10 by frequency (2+ occurrences)
  - Reveals workflow patterns: sequential bash, read-edit cycles, edit-test patterns

**Recently Added (2026-01-23)**:
- [x] Tool chains detection: sequences of consecutive single-tool calls (3+ length)
  - Shows turn ranges and tool sequence (e.g., "Turns 0-8: Grep → Read → Glob → ...")
  - Identifies parallelization opportunities more granularly than single count
  - Visible in both markdown and pretty output
- [x] Corrections/apologies detection: agent self-corrections and mistake acknowledgments
  - Detects: "I apologize", "my mistake", "let me fix", "actually" patterns
  - Categorized by type: Apology, Mistake, LetMeFix, Actually
  - Shows turn number and excerpt for each correction
  - Quality signal: fewer corrections = better prompts/tools

**Backlog - Analysis Features**:

1. **Cross-repo comparison** (MEDIUM VALUE)
   - Group sessions by repository (extract from path)
   - Compare metrics: tool usage, error rates, parallelization, costs
   - Table format: | Repo | Sessions | Tools | Errors | Cost |
   - Use case: identify which repos have friction, high costs, or inefficient workflows
   - Implementation: add `--by-repo` flag to stats command (already added to CLI)

2. **Ngram analysis** (HIGH VALUE for text understanding)
   - Extract common word sequences from assistant messages
   - Support n=2,3,4 (bigrams, trigrams, 4-grams)
   - Optional case-insensitive matching
   - Filter by message type (assistant, error messages, thinking)
   - Use cases:
     - Find common error messages across sessions
     - Identify repeated explanations/apologies
     - Detect boilerplate/templated responses
   - Implementation: tokenize text blocks, count ngram frequencies, rank by occurrence
   - Output: `normalize sessions show <id> --ngrams N [--case-insensitive]`
   - Example: "I apologize for" 5×, "let me fix" 8×, "failed to parse" 12×

3. **Token growth visualization** (HIGH VALUE) [DONE]
   - Track context size per turn, visualize growth curve
   - Flag bloat: warn when approaching context limits (e.g., 80K+ on Sonnet)
   - ASCII bar chart per turn: `Turn 1: ▓▓░░░░░░░░ 13K` → `Turn 40: ▓▓▓▓▓▓▓▓░░ 78K [!]`
   - Identify inflection points: when did context explode?
   - Use case: understand when/why sessions hit context limits
   - Implementation: add `per_turn_context: Vec<u64>` to SessionAnalysis

2. **Parallelization hints** (HIGH VALUE)
   - Beyond counting: show specific turns with sequential independent calls
   - Example: `Turn 12: ⚠️ Could parallelize: Read(foo.rs) → Read(bar.rs) → Read(baz.rs)`
   - Estimate savings: "2 API calls, ~15s latency saved"
   - Detect common anti-patterns: sequential reads, sequential edits
   - Implementation: analyze tool_chains for parallelizable sequences

3. **File edit heatmap**
   - Which files churned most? `src/main.rs (5 edits), lib.rs (4 edits)`
   - Files read but never edited: potential test gaps
   - Files edited multiple times: fragile design or iterative refinement?
   - Cross-reference with error patterns: files with failed edits
   - Implementation: track Read/Edit/Write targets, build frequency map

4. **Cost breakdown** (MEDIUM VALUE)
   - Model-specific pricing with current rates
   - Show cache savings: `Cache saved $0.76 (32% reduction)`
   - Compare models: "If Opus 4.5: $8.91 (4x more expensive than Sonnet 4.5)"
   - Per-turn cost tracking: identify expensive operations
   - Implementation: add pricing table, calculate from token_stats

5. **Tool chain pattern analysis**
   - Detect common sequences across sessions: `Read → Edit → Bash (git): 12 occurrences`
   - Identify workflows: test/commit cycle, search-and-replace pattern
   - Suggest optimizations: "Grep → Read → Edit detected 3 times: consider Grep+Edit fusion"
   - Implementation: aggregate tool_chains across sessions, find frequent subsequences

6. **Cross-repo comparison**
   - Aggregate metrics across ecosystem projects
   - Compare tool usage: `normalize: 78% Edit, spore: 45% Bash, resin: 89% Read`
   - Compare parallelization: which repos have most sequential patterns?
   - Compare error rates: which repos have fragile tooling?
   - Implementation: `normalize sessions stats --by-repo` with repo detection

7. **Message filtering subcommand** (NEW REQUEST)
   - Filter session messages by type: user, assistant, system, tool_use, tool_result
   - Keyword search within message text: `normalize sessions show <id> --filter user --grep "test"`
   - Use cases:
     - Extract all user prompts: `normalize sessions show <id> --filter user`
     - Find tool errors: `normalize sessions show <id> --filter tool_result --errors-only`
     - Search assistant reasoning: `normalize sessions show <id> --filter assistant --grep "because"`
   - Output modes: full messages, excerpts, counts
   - Implementation: new `normalize sessions show <id> --filter <type> [--grep PATTERN]`
   - Combine with existing jq: `normalize sessions show <id> --filter tool_use --jq '.name'`

**Other Session Analysis Backlog**:
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
- `normalize sessions stats`: cross-session aggregates (session count, token hotspots, total usage)
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

### Agent / MCP
- Gemini Flash 3 prompt sensitivity: certain phrases ("shell", "execute", nested `[--opts]`) trigger 500 errors. Investigate if prompt can be further simplified to avoid safety filters entirely. See `docs/design/agent.md` for current workarounds.
- `normalize @agent` (crates/normalize/src/commands/scripts/agent.lua): MCP support as second-class citizen
  - Our own tools take priority, MCP as fallback/extension mechanism
  - Need to design how MCP servers are discovered/configured
- Context view management: extend/edit/remove code views already in agent context
  - Agents should be able to request "add more context around this symbol" or "remove this view"
  - Incremental context refinement vs full re-fetch
  - Blocked on: agent implementation existing at all

### CI/Infrastructure
- [x] `normalize analyze duplicate-blocks` returns non-zero exit when unapproved groups found. Allow files exist. Wire into CI with `--exclude '**/*.json' --exclude '**/*.lock'` to suppress generated-file noise.

## Known Issues

### normalize-languages: ast-grep test broken
The `ast_grep::tests::test_pattern_matching` test fails to compile due to API mismatch:
- `DynLang.parse()` method not found
- `ast_grep_core::tree_sitter::LanguageExt` trait may need explicit import or implementation
- Pre-existing issue, not caused by feature flag changes

## Deferred

- VS Code extension: test and publish to marketplace (after first CLI release)
- Remaining docs: prior-art.md, hybrid-loops.md

## Python Features Not Yet Ported

### Orchestration
- Session management with checkpointing
- Driver protocol for agent decision-making
- Plugin system (partial - Rust traits exist)
- Event bus, validators, policies
- PR review, diff analysis
- TUI (Textual-based explorer)
- DWIM tool routing with aliases

### LLM-Powered
- Edit routing (complexity assessment → structural vs LLM)
- Summarization with local models
- Working memory with summarization

### Memory System
See `docs/design/memory.md`. Core API: `store(content, opts)`, `recall(query)`, `forget(query)`.
SQLite-backed persistence in `.normalize/memory.db`. Slots are user-space (metadata), not special-cased.

### Local NN Budget (from deleted docs)
| Model | Params | FP16 RAM |
|-------|--------|----------|
| all-MiniLM-L6-v2 | 33M | 65MB |
| distilbart-cnn | 139M | 280MB |
| T5-small | 60M | 120MB |

Pre-summarization tiers: extractive (free) → small NN → LLM (expensive)

### Usage Patterns (from dogfooding)
- Investigation flow: `view .` → `view <file> --types-only` → `analyze --complexity` → `view <symbol>`
- Token efficiency: use `--types-only` for architecture, `--depth` sparingly

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
