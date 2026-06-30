# normalize CLI — actual crate → service ownership map

Read-only investigation, 2026-06-29. Grounds a CLI taxonomy redesign organized by **crate ownership** (the stated rule: "a crate that owns a subcommand includes its own `#[cli]` service, report structs, and `OutputFormatter`; the main `normalize` crate just mounts them").

All evidence is from source under `/home/me/git/rhizone/normalize`. The built binary `target/debug/normalize` (current as of this session) was used to enumerate leaf commands.

---

## 1. Every workspace crate that exposes a `#[cli]` service

`grep -rl '#\[cli' crates/*/src/` yields exactly these crates with a service:

| Crate | Service struct | Standalone `#[cli(name=…)]` | Mounted in `NormalizeService`? | As verb | Leaf commands |
|-------|---------------|------------------------------|-------------------------------|---------|---------------|
| `normalize-budget` | `BudgetService` | (method-level only) | **yes** (`mod.rs:71`) | `budget` | measure, add, check, update, show, remove |
| `normalize-cfg` | `CfgService` | `name = "cfg"` | **yes** (`mod.rs:72`) | `cfg` | cfg (single leaf — `cfg cfg`) |
| `normalize-knowledge-graph` | `KgCliService` | `name = "normalize-knowledge-graph"` | **yes** (`mod.rs:73`) | `kg` | read, write, walk |
| `normalize-ratchet` | `RatchetService` | (re-exported) | **yes** (`mod.rs:74`) | `ratchet` | measure, add, check, update, show, remove |
| `normalize-rules` | `RulesService` | (mounted) | **yes** (`mod.rs:75`) | `rules` | list, run, enable, disable, show, tags, add, update, remove, setup, validate, compile, test, test-fixtures |
| `normalize-facts` | `FactsCliService` | `name = "normalize-facts"` | **NO** | — | (standalone only; see §3) |
| `normalize-filter` | `FilterCliService` | `name = "normalize-filter"` | **NO** | — | matches, aliases (standalone only) |
| `normalize-syntax-rules` | `SyntaxRulesService` | `name = "normalize-syntax-rules"` | **NO** | — | run, list (standalone only) |

**Key fact:** of the 8 crates that define a `#[cli]` service, only **5 are mounted** into the main binary (budget, cfg, kg, ratchet, rules). The other three (`normalize-facts`, `normalize-filter`, `normalize-syntax-rules`) define a service named for a *standalone binary* (`#[cli(name = "normalize-facts")]` etc.) that the main `normalize` binary never mounts. The main crate re-implements/reroutes those surfaces itself (see §3).

---

## 2. The main `normalize` crate's own commands

The main crate's CLI lives in `crates/normalize/src/service/*.rs` (`#[cli]` services) backed by report/compute code in `crates/normalize/src/commands/`.

### Sub-services defined in the main crate (`NormalizeService` fields, `mod.rs:60–80`)

| Field | Service (file) | Verb (`#[cli(name)]`) | Leaf commands |
|-------|----------------|------------------------|---------------|
| `analyze` | `analyze.rs` `AnalyzeService` | `analyze` | health, all, summary, liveness, effects, exceptions, docs, architecture, coupling-clusters, activity, repo-coupling, cross-repo-health, security, skeleton-diff |
| `rank` | `rank.rs` `RankService` | `rank` | complexity, ceremony, length, uniqueness, call-complexity, duplicates, duplicate-types, fragments, size, density, imports, surface, depth-map, layering, module-health, files, hotspots, coupling, ownership, contributors, test-ratio, test-gaps, budget |
| `trend` | `trend.rs` `TrendService` | `trend` | multi, complexity, length, density, test-ratio |
| `view` | `view.rs` `ViewService` | `view` | view, chunk, referenced-by, list, references, history, dependents, trace, graph, import-path, blame |
| `structure` | `facts.rs` `FactsService` | `structure` | rebuild, stats, files, packages, query, test-fixtures |
| `edit` | `edit.rs` `EditService` | `edit` | history, delete, replace, swap, insert, rename, undo, redo, goto, batch, move, introduce-variable, inline-variable, add-parameter, inline-function, extract-function |
| `syntax` | `syntax.rs` `SyntaxService` | `syntax` | ast, query, node-types |
| `tools` | `tools.rs` `ToolsService` | `tools` | lint, test |
| `context` | `context.rs` `ContextService` | `context` | query (default/hidden), migrate |
| `config` | `config.rs` `ConfigService` | `config` | schema, show, validate, set |
| `package` | `package.rs` `PackageService` | `package` | info, list, tree, why, outdated, audit |
| `sessions` | `sessions.rs` `SessionsService` | `sessions` | list, show, analyze, stats, ngrams, messages, subagents, patterns, parallelization, heatmap, cost, plans, mark, unmark |
| `daemon` | `daemon.rs` `DaemonService` | `daemon` | (lifecycle/run/status/roots) |
| `grammars` | `grammars.rs` `GrammarService` | `grammars` | install, list, … |
| `generate` | `generate.rs` `GenerateService` | `generate` | types, cli-snapshot |
| `guide` | `guide.rs` `GuideService` | `guide` | step-by-step guides |
| `serve` | `serve.rs` `ServeService` | `serve` | MCP/HTTP/LSP |

### Root leaf commands (methods directly on `NormalizeService`, `mod.rs`)

`grep`, `aliases`, `init`, `update`, `translate`, `docs`, `sync`, `ci` — these are top-level verbs with no sub-service and (mostly) no owning feature crate. Backing code in `commands/`: `text_search.rs` (grep), `aliases.rs`, `init.rs`, `update.rs`, `translate.rs`, `service/docs.rs`, `sync.rs`, `ci.rs`.

---

## 3. The critical mismatches: current verb ≠ owning crate

### 3a. analyze / rank / trend are ONE body of code in the main crate — not three crates

- `analyze.rs` imports every report from `crate::commands::analyze::*` (`analyze.rs:3–14`).
- `rank.rs` imports every report from `crate::commands::analyze::*` and `crate::analyze::*` (`rank.rs:5–30`).
- `trend.rs` was carved out of `AnalyzeService` (was `complexity-trend`, `length-trend`, … per `service/SUMMARY.md`).

All three services live in `crates/normalize/src/service/`, and **all of their command code lives in the single directory `crates/normalize/src/commands/analyze/`**. There is no crate boundary between analyze, rank, and trend. The split is a pure main-crate mounting choice over one pile of command modules.

**Where the metric commands actually live — verified:**
Every one of complexity, length, duplicates, ceremony, size, density, hotspots, coupling, ownership, imports, architecture, health, summary, security, docs is a module file under `crates/normalize/src/commands/analyze/` (e.g. `commands/analyze/complexity.rs`, `…/duplicates.rs`, `…/architecture.rs`, `…/report.rs` for `SecurityReport`/`AnalyzeReport`). The *computation* is delegated to pure library crates with **no `#[cli]`**: `normalize-analyze`, `normalize-architecture`, `normalize-metrics`, `normalize-code-similarity`, `normalize-graph`, `normalize-deps`, `normalize-semantic`, `normalize-scope` (all confirmed: 0 cli-files each).

So: **`normalize-analyze` is NOT the owner of the analyze commands.** It is a compute library. The CLI surface for analyze AND rank AND trend is owned by the main crate. There is no "normalize-analyze owns analyze, something else owns rank" boundary — it was always one crate (the main one).

**Consequence for by-crate taxonomy:** by-crate ownership *cannot* justify the analyze/rank/trend split, because they are one crate. By-crate would either collapse them to a single verb (with topics/subcommands) or leave the split as an editorial choice with no crate backing it.

### 3b. The three defined-but-unmounted services

- `normalize-facts::FactsCliService` (`#[cli(name="normalize-facts")]`) is **not** mounted. The main binary's `structure` verb is the *separate* `service/facts.rs::FactsService` (`#[cli(name="structure")]`). Two parallel facts services exist; only the main-crate one is wired into `normalize`.
- `normalize-filter::FilterCliService` (`matches`, `aliases`) is **not** mounted; the main crate only re-exports the `Filter` *type* (`src/filter.rs: pub use normalize_filter::*`) and implements `aliases` itself as a root leaf.
- `normalize-syntax-rules::SyntaxRulesService` (`run`, `list`) is **not** mounted; syntax-rule running is reached through `normalize-rules::RulesService` instead.

This is a partial migration: feature crates were given standalone `#[cli]` services, but the main binary still uses its own copies / routes around them.

### 3c. Per-verb backing — is each a crate or a main-crate grouping?

| Verb | Backed by own crate? | Notes |
|------|----------------------|-------|
| `view` | No — main crate `service/view.rs` | spans `commands/view/`, facts index |
| `structure` | Main crate `service/facts.rs` (parallel `normalize-facts` crate exists but unmounted) | crate=verb *almost* — wrong copy mounted |
| `syntax` | No — main crate `service/syntax.rs` | grammar inspection |
| `trend` | No — main crate (carved from analyze) | same code body as analyze/rank |
| `grep` | No — main crate root leaf (`text_search.rs`) | |
| `ci` | No — main crate root leaf (`commands/ci.rs`) | facade over rules engines (see §5) |
| `kg` | **Yes** — `normalize-knowledge-graph` | crate = verb ✓ |
| `budget` | **Yes** — `normalize-budget` | crate = verb ✓ |
| `ratchet` | **Yes** — `normalize-ratchet` | crate = verb ✓ |
| `rules` | **Yes** — `normalize-rules` | crate = verb ✓ |
| `context` | No — main crate `service/context.rs` (compute lib `normalize-context` has no cli) | |
| `sessions` | No — main crate `service/sessions.rs` (compute lib `normalize-session-analysis` has no cli) | |
| `edit` | No — main crate `service/edit.rs` (compute lib `normalize-refactor`, `normalize-shadow` have no cli) | |
| `daemon` | No — main crate | |
| `config` | No — main crate | |
| `cfg` | **Yes** — `normalize-cfg` | crate = verb ✓ but single leaf `cfg cfg` (redundant nesting) |
| `generate` | No — main crate (`normalize-typegen`/`normalize-openapi` compute libs, no cli) | |
| `guide` | No — main crate | |

**Clean crate=verb today: kg, budget, ratchet, rules, cfg (5).** Everything else is a main-crate grouping.

---

## 4. "One crate = one top-level subcommand" — the resulting verb set

If the rule were applied literally to the crates that currently expose CLI:

**From feature crates (already crate-owned):**
`budget`, `cfg`, `kg`, `ratchet`, `rules` — and, if their unmounted services were honored, `structure` (← `normalize-facts`), `filter` (← `normalize-filter`), and a syntax-rules verb (← `normalize-syntax-rules`).

**From the main crate (no feature crate owns them — they stay as main-crate top-level verbs):**
`view`, `syntax`, `edit`, `context`, `sessions`, `config`, `package`, `daemon`, `grammars`, `generate`, `guide`, `serve`, `tools`, plus root leaves `grep`, `aliases`, `init`, `update`, `translate`, `docs`, `sync`, `ci`, **and one verb for the analyze/rank/trend body** (since it is one crate, by-crate gives it one verb).

### Commands currently mounted under the WRONG verb relative to their owning crate

1. **analyze / rank / trend** — three verbs, one crate, one `commands/analyze/` directory. By-crate ownership has no basis for the 3-way split; it implies **one verb** (call it analysis) with the current verbs demoted to topic groups, OR an explicit editorial split that crate-ownership does not justify.
2. **`structure`** — mounted from the main crate's `service/facts.rs`, while the `normalize-facts` crate ships its own `FactsCliService` (`#[cli(name="normalize-facts")]`) that is never mounted. The verb exists but is backed by the *wrong copy*; by-crate says the `normalize-facts` service should be the one mounted.
3. **`aliases`** (root leaf) and the filter `matches` command — `normalize-filter` owns a `FilterCliService` with exactly these, unmounted. By-crate says these belong to a `filter` verb owned by `normalize-filter`, not a main-crate root leaf.
4. **syntax-rule running** — reachable via `rules`, but `normalize-syntax-rules` ships its own unmounted `run`/`list`. Either consolidate into `normalize-rules` (and delete the dead service) or honor the crate. Current state is duplicated surface.

---

## 5. Honest problem cases for the by-crate principle

**Grab-bag crates / heterogeneous surfaces.** The main `normalize` crate is itself the grab-bag: it owns view, syntax, edit, context, sessions, config, package, daemon, grammars, generate, guide, serve, tools, the analyze/rank/trend body, and 8 root leaves. By-crate ownership does *not* clean this up, because none of these have been extracted to feature crates — the architecture rule ("crate owns its subcommand") has only been followed for budget/cfg/kg/ratchet/rules. The principle describes an aspiration the codebase is ~25% of the way toward.

**A single concept fragmented across crates.** Rule running is split: `normalize-rules` (mounted as `rules`), `normalize-syntax-rules` (unmounted), and `normalize-native-rules`/`normalize-facts-rules-*` (compute libs). A naive crate=verb mapping would fragment "rules" into multiple verbs; the *right* answer is the current consolidation under one `rules` verb owned by `normalize-rules` — i.e. by-crate must NOT be applied to the rule-engine crates individually.

**Concept split that by-crate would MERGE: analyze/rank/trend.** Because they are one crate, by-crate collapses them. Whether that is desirable is an editorial/UX call, not a crate-boundary call — the current 3-way split has zero crate backing.

**Main-crate orphan commands.** `grep`, `init`, `update`, `translate`, `docs`, `sync`, `aliases`, `ci` have no owning feature crate. Under by-crate ownership they correctly **stay as main-crate top-level verbs** — the rule explicitly allows the main crate to host commands "with no standalone value and no home elsewhere." No problem here, but it means by-crate does not produce a fully crate-derived verb set; a residual of main-crate verbs always remains.

**`ci` as a facade.** `ci` (`commands/ci.rs`) runs the rule engines in sequence and exits non-zero on errors — it is a meta/orchestration command spanning rules (and conceptually budget/ratchet checks). If budget/ratchet/rules are each their own crate=verb (they are, except budget/ratchet checks aren't wired into `ci` today — `ci` runs the three *rule* engines, per `service/SUMMARY.md`), then `ci` survives **only as a meta-command** ("run all checks"), not as a facade owned by any single crate. It belongs in the main crate as a cross-cutting orchestrator, which is consistent with the by-crate principle (orchestration of multiple crates lives in the mounting crate).

**`cfg` redundant nesting.** `normalize-cfg` is a clean crate=verb, but its only leaf is `cfg cfg` — the verb and its single subcommand share a name. By-crate is satisfied; the UX wart (double `cfg`) is orthogonal.

---

## 6. Assessment: does by-crate-ownership produce a clean taxonomy?

**Partly — with two real snags.**

- For the **stateful-config commands** (budget, ratchet, rules, kg, cfg) by-crate is already true and clean: each is a cohesive crate with a verb-shaped command set. Extending the same treatment to `structure`/`filter`/syntax-rules (finishing the partial migration — mount the crate services, delete the main-crate duplicates) would make those clean too.

- **Snag 1 — analyze/rank/trend has no crate boundary.** This is the largest command surface (≈40 leaf commands) and it is one crate's worth of code (the main crate, computing via no-CLI libraries). By-crate ownership gives it exactly one verb and cannot adjudicate the current 3-way split. Any taxonomy that keeps analyze/rank/trend separate is making an editorial UX decision, not following crate ownership. This is the central tension the redesign must resolve: either (a) collapse to one verb with topic subcommands, or (b) keep the split but acknowledge it is verb-design, not crate-design, and stop citing the crate-ownership rule to justify it.

- **Snag 2 — the main crate is an irreducible grab-bag.** view, syntax, edit, context, sessions, config, package, daemon, grammars, generate, guide, serve, tools, and 8 root leaves all live in `normalize` with no feature crate. By-crate ownership leaves all of these as main-crate top-level verbs. So the verb set is never purely "one crate one verb" — it is "5–8 feature-crate verbs + a large residual of main-crate verbs." That residual is fine per the rule (commands with no standalone home stay in `normalize`), but it means by-crate-ownership is a *partial* organizing principle, not a complete taxonomy generator.

**Bottom line:** by-crate ownership cleanly fixes the *mounting bugs* (unmounted facts/filter/syntax-rules services, wrong `structure` copy) and validates the 5 stateful-config verbs, but it does **not** by itself produce a complete taxonomy — it under-determines both the analyze/rank/trend grouping (one crate) and the large main-crate residual (no crates). The redesign needs a second, UX-driven axis (topics within the analysis surface) layered on top of crate ownership.

---

## Appendix: evidence index

- Workspace members: `Cargo.toml` `members = [...]`.
- `#[cli]` service inventory: `grep -rl '#\[cli' crates/*/src/`.
- Sub-service composition: `crates/normalize/src/service/mod.rs:60–145`.
- analyze/rank report origins: `service/analyze.rs:3–14`, `service/rank.rs:5–30, 350` (`normalize_analyze::ranked::compute_ranked_diff` — library use, not command ownership).
- Unmounted standalone services: `normalize-facts/src/service.rs:147` (`name="normalize-facts"`), `normalize-filter/src/service.rs:101`, `normalize-syntax-rules/src/service.rs:173`.
- Library crates with 0 cli-files: normalize-analyze, -architecture, -metrics, -code-similarity, -graph, -deps, -semantic, -scope, -context, -session-analysis, -refactor.
- Root leaf verbs: `service/mod.rs` `#[cli]` methods at lines 291 (grep), 339 (aliases), 396 (init), 522 (update), 662 (translate), 811 (docs), 872 (sync), 1183 (ci).
- Leaf command lists: `target/debug/normalize <verb> --help`.
