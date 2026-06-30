# 00 — Authoritative FULL-INVERSION Plan: normalize CLI Taxonomy by Crate Ownership

*2026-06-30. Decision record + concrete command→owning-crate mapping. Implementation
spec, not implementation. Branch: `feat/cli-globals-pretty-wiring`.*

**This supersedes `00-retree-plan.md`** (the output-shape plan). The decision changed:
instead of organizing the analysis surface by output *shape* (rank/view/check/trend/
overview), we **push CLI ownership DOWN into the compute crates** so that the top-level
subcommand *is* the owning crate, per the project's own architecture rule:

> "A crate that owns a subcommand includes its own `#[cli]` service, report structs, and
> `OutputFormatter`; the main `normalize` crate just mounts them."

The axis decision is **made and not relitigated here.** What this document does is trace
every analyze/rank/trend/structural-view command to the crate that actually computes it,
then design the mounting moves that follow.

---

## 0. The central honest finding (READ FIRST — it bounds everything below)

**Inversion cleanly fixes the crate-owned commands and the mounting bugs, but it does NOT
by itself dissolve the analyze/rank/trend editorial split — because the bulk of that
surface has no owning compute crate.**

Tracing the compute (not the imports — the *algorithms*) shows three distinct realities:

1. **A minority of commands are genuinely owned by a compute crate** (architecture,
   graph, code-similarity, facts). These invert cleanly: move the `#[cli]` service into
   the crate, mount the crate as a verb. Mechanical, no taste calls.

2. **The metric core of `rank` has no compute crate.** complexity / length / ceremony /
   density / uniqueness / imports / surface / module-health / size / files / test-ratio /
   test-gaps / purposes(`rank budget`) — their computation lives **in the main crate**:
   `crates/normalize/src/analyze/{complexity,function_length,test_gaps}.rs` and the
   per-command modules in `crates/normalize/src/commands/analyze/*.rs`, which call
   `normalize-languages` (parsing) and `normalize-analyze` (ranking *infrastructure* —
   `RankEntry`, scoring, table render, diff) but **no per-metric compute crate**.
   `normalize-analyze` is the ranking-infra crate, **not** the owner of the metric
   commands. `normalize-metrics` is an aggregate/filter helper used **only by
   `normalize-budget` and `normalize-ratchet`** — it does not own any rank command either.

3. **The git-history cluster, the dashboards, the security/docs/skeleton-diff commands,
   and `trend` also have no compute crate** — their logic is `commands/analyze/
   git_history.rs`, `clusters.rs`, `security.rs`, `docs.rs`, `skeleton_diff.rs`,
   `summary.rs`, `trend.rs`, all in the main crate.

**Consequence:** there is no `metrics` verb to be had (the task's hoped-for
`normalize-metrics` verb does not exist as a compute owner), and `rank`/`trend` cannot be
dissolved by crate-ownership. They remain **main-crate verbs** holding the residual that
no crate owns — exactly the "irreducible main-crate grab-bag" the ownership map flagged
(`crate-ownership-map.md` §6, Snag 2). The drift that originally motivated the redesign
(H-4/H-5: metric commands silently moving `analyze`→`rank`) is **in this residual**, so
inversion does not fix that specific drift; it fixes a different, real problem (mounting
bugs + crate-owned commands sitting under main-crate editorial verbs).

**STOP-and-flag (the task asked for this explicitly):** the metric/git-history/dashboard
residual is the "command that composes so many crates / has no single owner" case. We do
**not** force it into a crate verb. Two honest options for the human (see §7, Open
Question A):

- **A1 (recommended, smaller):** Accept that `rank` and `trend` stay main-crate verbs for
  the residual. Inversion pulls the crate-owned families out (`graph`, `architecture`,
  `similarity`, dataflow→`structure`), fixes the mounting bugs, and shrinks `analyze` to a
  residual that still needs an editorial home (the shape plan's job — keep it parked).
- **A2 (larger, a precondition phase):** First **extract** the metric compute out of the
  main crate into a `normalize-metrics`-family crate (grow `normalize-metrics` or add a new
  crate absorbing `src/analyze/` + the per-metric `commands/analyze/*` compute +
  `git_history.rs`). *Then* the inversion yields a real `metrics` verb. This is a large
  extraction (≈18 commands' worth of compute, plus git plumbing), not a mounting move, and
  it is the only way crate-ownership reaches the rank core.

Everything below designs **A1's reachable inversions** in full, and specifies A2 as a
flagged follow-on so the human can choose.

---

## 1. Task 1 — command → owning compute crate (ground truth)

Traced by reading each module under `crates/normalize/src/commands/analyze/`,
`crates/normalize/src/analyze/`, and `service/{analyze,rank,trend,view}.rs`, and
confirming where the *algorithm* lives (not merely which crate is imported). "Owner" =
the crate that would hold the `#[cli]` method after inversion.

### 1.1 Genuinely crate-owned (invertible now)

| Command (today) | Owning compute crate | Single / composes | Evidence |
|---|---|---|---|
| `analyze architecture` | **normalize-architecture** | composes (`+ normalize-analyze` ranking, `+ normalize-languages`) — architecture is the owner | `commands/analyze/architecture.rs`; `normalize-architecture/src/lib.rs` `compute_coupling_and_hubs`, `ImportGraph`, `Cycle`, `HubModule`, `LayerFlow` |
| `rank depth-map` | **normalize-architecture** | single | `commands/analyze/depth_map.rs` → `normalize_architecture` |
| `rank layering` | **normalize-architecture** | single | `commands/analyze/layering.rs` → `normalize_architecture` |
| `view graph` | **normalize-graph** | single (structs already in the crate) | `GraphReport` is `normalize-graph/src/lib.rs:365` w/ `OutputFormatter`; `tarjan_sccs`, `find_bridges`, `find_diamonds` |
| `view dependents` | **normalize-graph** | single | `DependentsReport` is `normalize-graph/src/lib.rs:166` w/ `OutputFormatter`; `find_dependents`, `BlastRadius` |
| `view import-path` | **normalize-graph** | single | `ImportChain`, shortest-path in `normalize-graph` |
| `rank duplicates` | **normalize-code-similarity** | composes (`+ normalize-facts`, `+ normalize-languages`) — similarity is owner | `commands/analyze/duplicates.rs`; `compute_minhash`/`lsh_band_hash`/`jaccard_estimate` |
| `rank fragments` | **normalize-code-similarity** | composes (`+ normalize-analyze` ranking) | `commands/analyze/fragments.rs` → `normalize_code_similarity` |
| `rank duplicate-types` | **normalize-code-similarity** (data) / main-crate view | rendering of duplicates data | `commands/analyze/duplicates_views.rs` (imports only `normalize_analyze` ranking; underlying clone data is similarity's) |
| `view effects` | **normalize-facts** | single (dataflow query over index) | `commands/analyze/effects.rs` → `normalize_facts` |
| `view exceptions` | **normalize-facts** | single | `commands/analyze/exceptions.rs` → `normalize_facts` |
| `analyze liveness` (`view liveness`) | **normalize-facts** | single | `commands/analyze/liveness.rs` → `normalize_facts` |
| `rank call-complexity` | **normalize-facts** (call graph) + main-crate ranking | composes | `commands/analyze/call_complexity.rs` → `normalize_analyze` + `normalize_facts` |
| `structure *` (rebuild/stats/files/packages/query/test-fixtures) | **normalize-facts** | single — but the **wrong copy is mounted** (bug §4.1) | `normalize-facts/src/service.rs` `FactsCliService` (`name="normalize-facts"`, **unmounted**); main crate runs its own `service/facts.rs` (`name="structure"`) |

### 1.2 NO owning compute crate — compute lives in the main crate (the flagged residual)

These call `normalize-languages` (parsing) and/or `normalize-analyze` (ranking infra) and
`git_history.rs`, but **no crate owns their algorithm**. Owner column = "main crate".

| Command(s) | Where the compute is | Owner |
|---|---|---|
| `rank complexity / length / test-gaps` | `crates/normalize/src/analyze/{complexity,function_length,test_gaps}.rs` | **main crate** |
| `rank ceremony / density / uniqueness / imports / surface / module-health / size / files / test-ratio` | `commands/analyze/*.rs` (compute inline, parse via `normalize-languages`) | **main crate** |
| `rank budget` (purposes) | `commands/analyze/budget.rs` (line-count classification; `normalize_analyze` ranking only) | **main crate** |
| `rank hotspots / coupling / ownership / contributors` | `commands/analyze/git_history.rs` + `git_utils.rs` | **main crate** |
| `analyze activity / repo-coupling / cross-repo-health / coupling-clusters` | `commands/analyze/{activity,repo_coupling,cross_repo_health,coupling_clusters,clusters}.rs` (git history; no compute crate) | **main crate** |
| `analyze security / docs / skeleton-diff` | `commands/analyze/{security,docs,skeleton_diff}.rs` | **main crate** |
| `trend multi / complexity / length / density / test-ratio` | `commands/analyze/trend.rs` (composes metrics over git history) | **main crate** |

### 1.3 Composers / dashboards (compose MANY crates — flagged, no single owner)

| Command | Composes | Owner |
|---|---|---|
| `analyze health` | architecture + graph + similarity + metrics + facts + git | **main crate** (cross-cutting) |
| `analyze summary` | same | **main crate** |
| `analyze all` | same (≈ `health`, per audit M-4) | **main crate** |

These are legitimate cross-cutting compositions — CLAUDE.md explicitly puts orchestration
of multiple crates in the mounting crate. They stay main-crate (§2.5).

---

## 2. Task 2 — the inverted verb set

### 2.1 Compute-crate verbs (NEW `#[cli]` services pushed down — reachable now)

| Crate | Proposed verb | Taste call? | Commands it gains | Notes |
|---|---|---|---|---|
| **normalize-graph** | `graph` | clean — keep | `graph` (=`view graph`), `dependents`, `import-path` | structs+`OutputFormatter` already in the crate; just gate behind `cli` + add `#[cli]` service. Lowest blast. |
| **normalize-architecture** | `architecture` | **FLAG** — long; alt `arch` | `architecture`, `depth-map`, `layering` | `arch` is terser but less guessable. Recommend `architecture` w/ `arch` hidden alias. |
| **normalize-code-similarity** | `similarity` | **FLAG** — alt `clones`, `dupes`, `similar` | `duplicates`, `duplicate-types`, `fragments` | crate detects clones via MinHash/LSH. `similarity` is descriptive but verbose; `clones` is punchier. Recommend `similarity` w/ `clones` alias. |
| **normalize-facts** | `structure` (keep name) | clean — keep | mount the *real* `FactsCliService`; absorb dataflow `effects`/`exceptions`/`liveness` | replaces the stale main-crate copy (§4.1). |
| **normalize-semantic** | `search` | **FLAG** — alt `semantic` | semantic code search (currently orphaned) | see orphan verdict §4.4. `search` reads as a task; `semantic` names the mechanism. Recommend `search`. |
| **normalize-filter** | `filter` | clean — keep | `matches`, `aliases` | mount the unmounted `FilterCliService` (§4.2). |

### 2.2 Already-correct crate verbs (KEEP — no change)

`budget`, `cfg`, `kg`, `ratchet`, `rules` — each already a mounted crate-owned `#[cli]`
service. (cfg gets the double-`cfg` collapse, §4.5; rules absorbs the syntax-rules
consolidation, §4.3.)

### 2.3 Main-crate residual verbs (no owning crate — stay top-level in `normalize`)

Single-file inspection and CLI-wiring verbs with no standalone compute crate:

`view` (single-file/symbol inspection — keeps the source/call-graph/git/trace families),
`edit`, `sessions`, `context`, `grep`, `init`, `update`, `translate`, `docs`, `sync`,
`daemon`, `generate`, `guide`, `package`, `serve`, `config`, `ci`, `syntax`, `tools`,
`grammars`.

**Plus the flagged residual (§0):** `rank` (metric + git-history cluster) and `trend`
remain main-crate verbs under option A1. Under A2 they become the extracted `metrics`
verb.

### 2.4 The `analyze` verb after inversion

Inversion empties `analyze` of its crate-owned members:
- `architecture` → `architecture` verb
- `liveness/effects/exceptions` → `structure` (dataflow over the index)
- (graph members were already under `view`)

What is **left in `analyze`** has no compute crate: `security`, `docs`, `skeleton-diff`,
`health`, `summary`, `all`, `activity`, `repo-coupling`, `cross-repo-health`,
`coupling-clusters`. So **inversion shrinks `analyze` but does not dissolve it.** Fully
dissolving it requires the editorial homes the shape plan provided (security/docs→`check`,
dashboards→`overview`, etc.). **Open Question A** governs whether we (A1) leave this
residual parked under a slimmed `analyze`, or (A2) extract compute and/or re-apply the
editorial split. Recommended: A1 now; revisit the residual once the crate-owned moves
land and the surface is smaller.

### 2.5 Cross-cutting compositions

**Dashboards (`health`/`summary`/`all`).** Compose architecture+graph+similarity+metrics+
facts. → a thin **main-crate composition verb**. Proposed name: **`overview`** (consistent
with the superseded plan; alternatives `report`, `dashboard`). Recommend a verb over a
flag because they are three *distinct* aggregate reports, not one report with a mode.
Collapse `all`→`overview --full` is a separate taste call (audit M-4: `all`≈`health`).

**`trend`.** Decision: **stays a main-crate cross-cutting verb** that composes the compute
crates over git history. Do **not** push a history/over-time mode into each compute crate.
Reasoning: (1) `trend`'s compute (`trend.rs`) is genuinely composite — it walks git refs
and re-runs metrics; the git-walk plumbing is a main-crate concern (CLAUDE.md: "normalize
knows about git" at the wiring layer, not in pure compute crates). (2) Scattering a
`--since`/history mode across N crates would duplicate the git-walk and the snapshot-diff
logic N times — the opposite of consolidation. (3) Pushing it down would also force the
metric residual (which has no crate) to grow a history mode it can't host. (4) It would
**not** be an enum-wrap, but it would be N parallel implementations of the same time-axis —
worse than one composer. Keep `trend` as the single time-series composer.

---

## 3. Task 3 — struct / wiring moves (per reachable crate verb)

**API-first invariant:** report data shapes do NOT change. Only ownership/mount moves.
`assert_output_formatter::<R>()` tests follow each struct to its new crate.

### 3.1 normalize-graph → `graph`

- **Already in the crate:** `GraphReport`, `DependentsReport` + their `OutputFormatter`
  impls (`lib.rs:166,365`). **Bug:** these are compiled unconditionally (no `cli` gate)
  and the crate depends on `normalize-output` unconditionally (§4.6).
- **Add to crate:** a `cli` feature (`cli = ["dep:server-less", "dep:normalize-output"]`,
  `default = ["cli"]` per convention); move the `normalize-output` dep + the two
  `OutputFormatter` impls behind `#[cfg(feature = "cli")]`; add a `#[cli(name="graph")]`
  `GraphService` with methods `graph`/`dependents`/`import-path`.
- **Move from main crate:** the wiring in `service/view.rs` (methods `graph`,
  `dependents`, `import_path`, display helpers at lines 113/121/542/600/631) and the
  re-export wrapper `commands/analyze/graph.rs`.
- **Main crate mounts:** `graph: normalize_graph::GraphService` on `NormalizeService`
  with `features = ["cli"]`.

### 3.2 normalize-architecture → `architecture`

- **Move into crate:** `ArchitectureReport` (`commands/analyze/architecture.rs`),
  `DepthMapReport` (`depth_map.rs`), `LayeringReport` (`layering.rs`) + their
  `OutputFormatter` impls.
- **Add to crate:** `cli` feature + `normalize-output` (gated); `#[cli(name=
  "architecture")]` service with `architecture`/`depth-map`/`layering`.
- **Main crate mounts** the service; deletes the three command modules' CLI bodies (compute
  helpers that remain main-crate-only, if any, move with the report).

### 3.3 normalize-code-similarity → `similarity`

- **Move into crate:** `DuplicatesReport`, `DuplicateTypesReport`, `FragmentsReport` +
  `OutputFormatter`.
- **Add:** `cli` feature + `normalize-output`; `#[cli(name="similarity")]` service with
  `duplicates`/`duplicate-types`/`fragments`.
- **Note:** these compose `normalize-facts`/`normalize-languages`; the service takes those
  as constructed dependencies (no global/env reads — CLAUDE.md hard constraint).

### 3.4 normalize-facts → `structure` (fix + absorb dataflow)

- **Mount the real `FactsCliService`** (`normalize-facts/src/service.rs`); **delete** the
  parallel main-crate `service/facts.rs` (`name="structure"`) after porting any methods it
  has that the crate service lacks. Rename the crate service `#[cli(name=
  "normalize-facts")]`→`name="structure"`.
- **Absorb dataflow:** move `LivenessReport`/`EffectsReport`/`ExceptionsReport` +
  `OutputFormatter` into the crate (or a `normalize-facts` cli submodule) and add
  `liveness`/`effects`/`exceptions` methods (they already query the facts index).
- **Activate** `normalize-facts` with `features = ["cli"]` in the main crate (today it is
  mounted with no features).

### 3.5 normalize-semantic → `search`

- **Already in the crate:** `SearchReport`, `ContextSearchReport` + `OutputFormatter`
  (`service.rs:55,251`), plus a full engine (`embedder`, `store`, `search`, `populate`).
- **Missing:** the `#[cli]` attribute is never applied; the crate is **not** a dependency
  of the main binary. Add a `#[cli(name="search")]` service wrapping the existing methods;
  add `normalize-semantic = { ..., features = ["cli"] }` to the main crate and mount it.

### 3.6 normalize-filter → `filter`

- **Already in the crate:** `FilterCliService` (`name="normalize-filter"`) with `matches`,
  `aliases`. Rename `name`→`filter`; mount with `features = ["cli"]` (today `["config"]`).
- **Remove from main crate:** the root-leaf `aliases` method that re-implements it
  (`service/mod.rs:339`); `aliases` becomes `filter aliases` (with a hidden top-level
  `aliases` transitional alias).

---

## 4. Task 4 — bug-cluster fixes + small fixes folded in

### 4.1 `structure` wrong-copy (covered §3.4)
Two parallel facts services exist; the unmounted crate copy is canonical. Mount it, delete
the main-crate duplicate.

### 4.2 `normalize-filter` unmounted (covered §3.6)
Mount `FilterCliService`; retire the main-crate `aliases` leaf.

### 4.3 `normalize-syntax-rules` fragmented rule-running
`SyntaxRulesService` (`run`, `list`) is defined but unmounted; syntax-rule running is
*also* reachable through `normalize-rules`. **Investigate result:** this is duplicated
surface, not two real owners. **Recommendation:** consolidate into the existing `rules`
verb (the single rule-engine home), and **delete** the standalone `SyntaxRulesService`
CLI scaffolding if `rules` already routes syntax rules (verify `rules run --type syntax`
exists). Do **not** add a separate top-level syntax-rules verb — that fragments "rules"
across two verbs, which the ownership map explicitly warns against. *Confirm the `rules`
path covers syntax rules before deleting* (one-command check during B6).

### 4.4 `normalize-semantic` orphan verdict
**Verdict: WIRE IT, do not delete.** Applying CLAUDE.md's "delete infra only if
speculative from the start" test: this is a **complete, non-speculative feature** — vector
embedder, SQLite vec store, populate pipeline, staleness tracking, rerank, two report
types with `OutputFormatter`, a config surface. It solves a real problem class (semantic
code search). The only missing piece is the `#[cli]` method + the mount. Wire it as the
`search` verb (§3.5). (If the human judges semantic search out of scope for normalize's
identity, that's a product call, not an "it was speculative" deletion — flag to the human
but default to wiring.)

### 4.5 `cfg cfg` → `cfg`
`normalize-cfg` is a clean crate-owned verb whose single leaf shares the verb name. Collapse
so the cfg method mounts directly as the verb (`normalize cfg <fn>`), not `cfg cfg`. Owner
stays `normalize-cfg`. (Divergence from the shape plan, which moved it to `view cfg`; under
crate-ownership it stays its own verb.)

### 4.6 `normalize-graph` ungated `OutputFormatter` (covered §3.1)
Gate the two impls + the `normalize-output` dep behind the new `cli` feature.

### 4.7 `edit history` → `edit log`
Rename the `edit history` subservice to `edit log` (resolves the `view history` collision,
audit M-3). Main-crate `service/edit.rs` / `history.rs`; mount `HistoryService` as `log`.

### 4.8 `rank budget` collision
The line-count breakdown (`commands/analyze/budget.rs`) has **no compute crate** — it is
main-crate residual. It stays under `rank`; **rename to `rank purposes`** to free the
`budget` word for the `normalize-budget` crate verb (audit H-2). Owner: main crate.

### 4.9 Transitional aliases
Every moved command path keeps a hidden one-release alias; removed at 1.0. **server-less
prerequisite:** add `#[cli(alias = "...")]` (hidden clap alias) to server-less — it has
`#[server(hidden)]` and `#[cli(name=…)]` but no `alias` attr. For **cross-verb moves**
(`analyze architecture`→`architecture architecture`, `view graph`→`graph`), keep the old
parent mounted as a `#[server(hidden)]` shim delegating to the new home for one release
(clap aliases are scoped to one parent). Fix the alias gap in server-less, not here.

### 4.10 Guide / help regression test
Land FIRST (R0). Parse each `guide` body (`const &str` in `service/guide.rs`) for
`normalize …` lines; resolve each against the live command tree; fail CI on any
unresolved example. Pair with a `--help`/topic snapshot test so moves can't re-break
guides (the H-4/H-5 failure mode).

### 4.11 CLAUDE.md stale crate count
Fix "**38 crates**" → **47 workspace members** (45 named + benches + xtask; 2
`publish=false`) in the root `CLAUDE.md` Publishing section (per the census).

---

## 5. Task 5 — batched execution plan

One compute-crate verb per batch. Each batch = move structs + add `cli` feature + add
`#[cli]` service + mount + hidden aliases + tests green + doc sync, in one commit. Keep
`cargo clippy --all-targets --all-features -- -D warnings && cargo test -q` green per
batch. Lowest blast first.

| Batch | Scope | Commands moved | Structs moved | Crates touched |
|---|---|---|---|---|
| **B0** | Guide/help regression + topic snapshot tests (§4.10); fix CLAUDE.md crate count (§4.11) | 0 | 0 | normalize |
| **B1 server-less prereq** | add `#[cli(alias=…)]` to server-less; publish; bump dep | 0 | 0 | server-less (separate repo) |
| **B2 `graph`** | gate `OutputFormatter` behind `cli`; add `GraphService`; mount; move `view graph/dependents/import-path` | 3 | 0 (already in crate) | normalize-graph, normalize |
| **B3 `architecture`** | move 3 reports into crate; `cli` feature + service; mount; retire `analyze architecture` mount | 3 | 3 | normalize-architecture, normalize |
| **B4 `similarity`** | move 3 reports; `cli` feature + service; mount | 3 | 3 | normalize-code-similarity, normalize |
| **B5 `structure` fix** | mount real `FactsCliService` (rename→`structure`); delete main-crate dup; absorb `liveness/effects/exceptions`; activate `features=["cli"]` | 3 + structure leaves | 3 | normalize-facts, normalize |
| **B6 `filter` + syntax-rules** | mount `FilterCliService` (rename→`filter`); retire `aliases` leaf; consolidate/delete syntax-rules dup into `rules` | 2 (+verify) | 0 | normalize-filter, normalize-syntax-rules, normalize-rules, normalize |
| **B7 `search`** | add `#[cli]` to `normalize-semantic`; mount as `search` | (new surface) | 0 | normalize-semantic, normalize |
| **B8 small fixes** | `cfg cfg`→`cfg`; `edit history`→`edit log`; `rank budget`→`rank purposes`; dashboards→`overview` (composition verb) | ~4 renames + 3 dashboards | 0 | normalize-cfg, normalize |
| **B9 alias sunset** | (at 1.0) remove all hidden aliases/shims | 0 | 0 | all touched |

### Blast radius (total, option A1)

- **Commands re-pathed:** ≈ 18 invocation strings change (graph 3, architecture 3,
  similarity 3, dataflow 3, structure mount-swap, filter 2, cfg 1, edit-log 1, rank-purposes
  1, dashboards 3) of ~165 (~11%).
- **Structs relocated:** ≈ 12 report structs move from `commands/analyze/` into their
  owning crates (graph's 2 already there).
- **Crates touched:** ~7 compute crates gain a `cli` feature + `#[cli]` service +
  `normalize-output` dep; the main crate sheds those command bodies and mounts the
  services; server-less gains an alias attr.
- **Untouched (A1):** the entire metric/git-history/dashboard residual under `rank`/`trend`
  — that is the flagged §0 surface, only reachable by A2's extraction phase.

### Doc sync (every batch, per the hard rule)

`docs/cli/`, `README.md`, `LLMS.md`, `docs/cli-design.md`, all `guide` bodies,
`CHANGELOG.md` (`[Unreleased]`), every touched `SUMMARY.md` (notably
`crates/normalize/src/service/`, `commands/analyze/`, and each crate gaining a service),
regenerate `cli-snapshot`. Fix the stale "38 crates"→"47" in `CLAUDE.md` (B0).

---

## 6. CI lint (ownership invariant)

Inversion has a natural compile-time invariant cheaper than the shape plan's marker
traits: **a top-level verb's `#[cli]` service must be defined in the crate the verb is
named for.** Enforce via a small `xtask`/native rule that parses the generated
`cli-snapshot` + the `NormalizeService` mount fields and asserts each mounted service's
type path resolves into the matching crate (`graph` → `normalize_graph::*`, etc.). This
catches re-accretion of CLI bodies back into the main crate's `commands/` for a crate-owned
verb. The metric residual verbs (`rank`/`trend`) are explicitly exempt (allow-listed as
main-crate-owned) until/unless A2 extracts them.

---

## 7. Open naming / decision questions for the human (confirm before implementing)

**A. The §0 residual — the load-bearing decision.** The metric core of `rank` + the
git-history cluster + dashboards + `trend` have no compute crate. Choose:
  - **A1 (recommended):** accept `rank`/`trend` as main-crate verbs for the residual; do
    only the reachable crate-owned inversions (B2–B8). Cheapest; honest about ownership.
  - **A2:** first extract the metric compute into a `normalize-metrics`-family crate (large
    precondition), then mount a real `metrics` verb. Only path to dissolving the rank core
    by crate-ownership.

**B. Verb names (taste calls):**
  - `normalize-architecture` → **`architecture`** (vs `arch`).
  - `normalize-code-similarity` → **`similarity`** (vs `clones` / `dupes` / `similar`).
  - `normalize-semantic` → **`search`** (vs `semantic`).
  - dashboards composition verb → **`overview`** (vs `report` / `dashboard`); and collapse
    `all`→`overview --full`?

**C. `cfg` as its own verb vs `view cfg`.** Crate-ownership keeps `cfg` a `normalize-cfg`
verb (collapsing the double-`cfg`). The superseded shape plan moved it to `view cfg`.
Confirm: keep `cfg` as its own crate verb.

**D. `normalize-semantic` scope.** Wire it as `search` (default), or is semantic search
out of normalize's product scope? (Not a deletion-for-speculation call — the feature is
complete; this is a product judgment.)

**E. syntax-rules consolidation (§4.3).** Confirm `rules` already routes syntax rules so
the standalone `SyntaxRulesService` can be deleted rather than mounted as a second verb.

### Flagged soft spots (NOT forced)

- **The metric/git-history/dashboard residual (§0)** — the central one. Inversion does not
  reach it; it is the "no single owner" STOP case. Surfaced as Question A, not forced into
  a crate verb.
- **`rank duplicate-types`** — a rendering of similarity's clone data; its module imports
  only `normalize-analyze` ranking. Goes with `similarity` (data owner) but is borderline;
  flag if the human reads it as a separate concern.
- **dataflow under `structure`** — `liveness/effects/exceptions` query the facts index, so
  `structure` is the honest owner, but they read as "analysis" not "index introspection."
  Acceptable; cross-reference in help.

No command is forced into a crate verb it doesn't cohere with; the residual is flagged for
the human rather than jammed into a `metrics` verb that no crate backs.
