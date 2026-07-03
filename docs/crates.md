# Crate Registry

The canonical "what crate is for what" reference for the normalize workspace. This
**replaces the removed per-directory `SUMMARY.md` convention at the crate level** — one
durable place to answer "which crate owns X?" instead of re-deriving it from scratch in
every audit.

- **Source of truth for each purpose is the crate's `Cargo.toml` `description` field.** The
  one-liners here are seeded from those descriptions; keep the `description` accurate and
  this registry stays cheap to regenerate. Where a description is missing/unhelpful, the
  row's notes flag it (`FLAG: description`).
- **Namespace mapping** (which crate backs `normalize <verb>`) follows the CLI taxonomy
  inversion: authoritative plan in
  `docs/artifacts/cli-taxonomy-2026-06-29/00-inversion-plan.md` (FINAL SCOPE), reconciled
  decomposition roadmap in `docs/audit-2026-07-03-command-surface-decomposition.md`.
  Current mounts are read from `crates/normalize/src/service/mod.rs`.
- **Categories:** `cli-surface` (owns a `#[cli]` service mounted by main), `compute`
  (domain algorithms / data models, command surface still in main), `rules` (rule
  engines / rule data), `infra` (facts/index/git/languages/grammars substrate), `wiring`
  (output/derive/rank plumbing, build tooling).

**Workspace:** 49 members (47 crates in `crates/` + `xtask` + `benches`). Published:
46 crates at v0.3.2. `publish = false`: `normalize-grammars`, `xtask`, `benches`.

**Namespace legend:** *current* = mounted top-level verb today; *planned (inversion)* =
target verb per the inversion plan, not yet mounted; `—` = compute-only, command surface
lives in main.

---

## cli-surface — owns a `#[cli]` service mounted by main

| crate | purpose | namespace (current / planned) | key notes |
|---|---|---|---|
| `normalize` | Fast code intelligence CLI and library | (host binary) | Main crate: command dispatch, global flags, output backend, service composition, vendored CLIs (`rg`/`ast_grep`/`jq`), `view`/`edit`/`analyze`/`rank`/`trend`/`init`/`update`/`sync` residual. ~84k L; ~21k vendored (forced to stay). |
| `normalize-budget` | Diff-based budget system: track how much a codebase is allowed to change | `budget` | Reference "crate owns its subcommand" shape. |
| `normalize-cfg` | Control flow graph builder | `cfg` | Owns `cfg` verb; candidate home for the dataflow trio (liveness/effects/exceptions) — unresolved fork vs `normalize-facts`. |
| `normalize-knowledge-graph` | Persistent, addressable, queryable knowledge graph adjacent to code — unit CRUD, edge management, BFS traversal | `kg` | |
| `normalize-ratchet` | Metric regression-tracking (ratchet) system | `ratchet` | Uses `normalize-facts::FileIndex` directly (migration precedent). |
| `normalize-rules` | Rule orchestration and CLI service (syntax + fact + native + SARIF engines) | `rules` | Mounts the syntax/fact/native rule engines behind one verb. |
| `normalize-sessions` | `sessions` command surface: inspect and analyze AI agent session logs | `sessions` | Newest cli-surface crate; the clean proof case for the decomposition (main src −8k L). Deps: `normalize-chat-sessions` + `normalize-session-analysis`. |

## compute — domain algorithms / data models (command surface still in main unless planned)

| crate | purpose | namespace (current / planned) | key notes |
|---|---|---|---|
| `normalize-index` | Index acquisition (config-slice API) + import-graph construction | library (no verb) | **Enabler crate** unblocking B2+: feature crates acquire the index via `open`/`ensure_ready`/`require_import_graph` taking `&IndexConfig`+`&WalkConfig` (no `NormalizeConfig` dep). Owns `build_import_graph`/`ImportGraph` — moved here from `normalize-architecture` to break the `graph ↔ architecture` cycle. Deps: `normalize-facts`, `normalize-rules-config`, `normalize-core`. |
| `normalize-architecture` | Architectural metrics (coupling, cycles, layering, hubs) + the `architecture` verb | `architecture` | **Owns `architecture` verb (B3 done):** `architecture` (coupling/cross-imports/hub modules), `architecture layering` (import-direction compliance), `architecture depth-map` (depth + ripple risk) — carved out of `analyze architecture`/`rank layering`/`rank depth-map`. CLI surface (`ArchitectureService`, report structs, `OutputFormatter`) gated behind `cli` feature; library consumers of the pure algorithms build with `default-features = false`. Acquires the index via `normalize-index` (`require_import_graph`/`ensure_ready`, config slices); `--diff` uses `normalize-git`. Old `analyze`/`rank` paths kept as hidden transitional aliases for one release. |
| `normalize-graph` | Pure graph algorithms (SCC, bridges, diamonds, chains, transitive edges) + the `graph` verb | `graph` | **Owns `graph` verb (B2 done):** `graph` (module/symbol/type graph), `graph dependents`, `graph import-path` — carved out of `view graph`/`dependents`/`import-path`. CLI surface (`GraphService`, report structs, `OutputFormatter`) gated behind `cli` feature; library consumers of the pure algorithms build with `default-features = false`. Acquires the index via `normalize-index` (`require_import_graph`, config slices). Old `view` paths kept as hidden transitional aliases for one release. |
| `normalize-code-similarity` | Code similarity algorithms (MinHash LSH, normalized AST hashing, structural tokenization) + the `similarity` CLI verb behind `cli` | **owns `similarity`** (B4) | `similarity` (duplicates incl. `--mode clusters`), `similarity duplicate-types`, `similarity fragments`. Index-free (walks the filesystem). Pure algorithms build with `default-features = false`. `coupling-clusters` is git-temporal, NOT here (→ `history`, B9). |
| `normalize-semantic` | Semantic retrieval layer: vector embeddings over structurally-derived chunks | planned (inversion) `search` | Complete feature; only missing `#[cli]` + mount. Note `search` verb-name collision risk (open fork). |
| `normalize-filter` | File filtering with glob patterns and alias resolution | planned (inversion) `filter` | `FilterCliService` exists but is unmounted — mount to realize the verb. |
| `normalize-edit` | Structural code editing | — | Backs `edit`; service coupled to `crate::index`/shadow state (later migration). |
| `normalize-refactor` | Composable refactoring engine | — | |
| `normalize-scope` | Scope analysis engine using tree-sitter locals queries | — | |
| `normalize-deps` | Module dependency extraction (imports, exports, re-exports) | — | |
| `normalize-context` | Frontmatter-filtered context resolution: hierarchical `.normalize/context/` walk with YAML frontmatter matching | — | Backs `context`. |
| `normalize-typegen` | Polyglot type and validator generation from schemas | — | Backs `generate`. |
| `normalize-openapi` | OpenAPI client code generation | — | |
| `normalize-surface-syntax` | Surface-level syntax translation between languages via a common IR | — | Backs `translate`. |
| `normalize-cli-parser` | Parse CLI `--help` output from various frameworks | — | Fixtures excluded from workspace. |
| `normalize-tools` | Unified interface for external development tools (linters, formatters, type checkers) | — | Backs `tools`. |
| `normalize-ecosystems` | Project dependency management for multiple package ecosystems | — | |
| `normalize-manifest` | Manifest file parsing for programming language ecosystems | — | |
| `normalize-local-deps` | Local dependency discovery for programming language ecosystems | — | |
| `normalize-package-index` | Package index ingestion from distro and language registries | — | Backs `package`. |
| `normalize-chat-sessions` | Session log parsing for AI coding agents | — | Substrate for `normalize-sessions`; also `provenance`. |
| `normalize-session-analysis` | Session analysis metrics for AI coding agent logs | — | Substrate for `normalize-sessions`. |

## rules — rule engines / rule data types

| crate | purpose | namespace (current / planned) | key notes |
|---|---|---|---|
| `normalize-syntax-rules` | Syntax-based linting rules with tree-sitter queries | — (via `rules`) | Routed under `rules run --type syntax`; no separate verb. |
| `normalize-native-rules` | Native rule checks (check-refs, stale-docs, check-examples, ratchet, budget) | — (via `rules`) | `stale-summary`/`missing-summary` removed with the SUMMARY convention. |
| `normalize-facts-rules-api` | Data types for fact rules (Relations input, Diagnostic output) | — | |
| `normalize-facts-rules-interpret` | Interpreted Datalog rule evaluation for code facts | — | |
| `normalize-rules-config` | Shared rule configuration types (`RulesConfig`, `RuleOverride`) | — | |

## infra — facts/index/git/languages/grammars substrate

| crate | purpose | namespace (current / planned) | key notes |
|---|---|---|---|
| `normalize-facts` | Code fact extraction and storage library | planned (inversion) `structure` | Owns the index + cyclomatic core. `structure` is main-backed today (stale copy); plan mounts the real `FactsCliService` and absorbs the dataflow trio (B5). |
| `normalize-facts-core` | Core data types for normalize facts (symbols, imports, exports) | — | |
| `normalize-git` | Pure-Rust read-only git operations: repo open, blob read, tree walk, diff, blame, churn, history | — | Extracted 2026 (B1) to dedup gix helpers across budget/ratchet/semantic/native-rules/main. Future `normalize-git-history` (planned `history` verb, B8/B9) will depend on it. |
| `normalize-shadow` | Shadow git history tracking for edit operations | — | |
| `normalize-languages` | Tree-sitter language support and dynamic grammar loading | — | `GrammarLoader`; loads `*.scm` query files. |
| `normalize-language-meta` | Language metadata and capabilities | — | |
| `normalize-grammars` | Marker crate aggregating all tree-sitter grammar dependencies | — | `publish = false`. No code of its own — declares grammar deps so they link into the binary. |
| `normalize-module-resolve` | Module resolution infrastructure for cross-file analysis | — | |
| `normalize-path-resolve` | Path resolution and fuzzy matching | — | |
| `normalize-core` | Core traits and types for the normalize code intelligence system | — | Foundational shared traits. |

## wiring — output/derive/rank plumbing, build tooling

| crate | purpose | namespace (current / planned) | key notes |
|---|---|---|---|
| `normalize-output` | Output formatting for CLI commands | — | Defines `OutputFormatter` (main's `output.rs` is a re-export). Migrating report structs depend on this directly, not main. |
| `normalize-derive` | Derive macros for normalize | — | |
| `normalize-rank` | Shared entity types, ranking pipeline, and table rendering for `rank` commands | main-resident `rank` | Metric bucket stays main (seam eval A1); `RankEntry` CI lint holds against drift. |
| `normalize-metrics` | Shared metric primitives for ratchet and budget systems | — | Distinct from the AST-metric bucket; a `metrics`-family crate would collide with this name. |
| `xtask` | Build/dev automation tasks | (build) | `publish = false`. |
| `benches` | Benchmarks | (build) | `publish = false`. |

---

## Notes / flags for follow-up

- **`normalize-grammars`** — `Cargo.toml` description was the placeholder "Normalize"; fixed
  in this change to an accurate one-liner.
- **Vendored CLIs are NOT separate crates** — `rg`, `ast_grep`, and `jq` front-ends live in
  `crates/normalize/src/{rg,ast_grep,jq}/` (~21k L), forced to stay in main by the publish
  trilemma (`docs/audit-2026-07-02.md`). There is no `vendored` crate category.
- **Planned (inversion) rows are targets, not current state.** They mount as their own verb
  only after the corresponding batch (B2–B12) lands. `normalize-git-history` (planned
  `history` verb) does not exist yet.
- **Open forks** (dataflow-trio home cfg-vs-facts, metrics A1/A2, `search` collision) are
  tracked in `docs/audit-2026-07-03-command-surface-decomposition.md`, not here.
