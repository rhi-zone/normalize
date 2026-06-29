# Candidate B — Organize by Output Data-Shape

**Frame:** Apply normalize's own "API that happens to have a CLI" principle *literally*.
The top-level verb is a projection of the SHAPE of the typed value a command returns.
Two commands that return the same shape live under the same verb.

> Status: design only. Not committed. Grounded in the actual report structs read on
> 2026-06-29 (citations are absolute paths). Where I verified a struct directly it is
> marked ✓; the broader buckets are classified from the same evidence base.

---

## 1. The principle and the shape→verb taxonomy

**Principle (one sentence):** *The top-level verb names the shape of the typed data the
command returns — a command's home is determined by `what does its Report struct look
like`, never by its subject matter or by CLI ergonomics.*

### Shapes found in the actual return types

I read the report structs and found that every command's payload reduces to one of
**eight** shapes. The verb is the shape's name in the imperative/role form a user would
recognize.

| # | Shape | What the struct looks like (evidence) | Verb |
|---|-------|----------------------------------------|------|
| 1 | **RANKED-LIST** | `Vec<Entry>` sorted by a metric/score; entries impl `RankEntry`. e.g. `FunctionComplexity`, `HotspotsReport{ entries: Vec<..> }`, `DepthMapReport{ entries: Vec<DepthEntry> }` ✓ | `rank` |
| 2 | **GRAPH** | nodes+edges, cycles/SCCs, hubs, centrality, rendered flowcharts. `GraphReport{ sccs, diamonds, bridges, longest_chains, dead_nodes }` ✓; `ArchitectureReport{ cross_imports, hub_modules, coupling_hotspots, … }` ✓; `CfgReport{ output, block_count, edge_count }` ✓ | `graph` |
| 3 | **ENTITY-VIEW** | one report about one file/symbol/value — body text or an outline of one target. `ViewReport` ✓, `ChunkedViewReport` ✓, `TraceReport{ trace: String }` ✓, `CfgReport` (one function) | `view` |
| 4 | **TREE** | hierarchical nesting (children inside nodes). `rank size` ("ncdu-style tree"), `view list`/outline, `package tree`, `edit history tree` | `tree` |
| 5 | **TIME-SERIES** | points over commits/time. `TrendReport{ snapshots: Vec<TrendSnapshot> }` ✓, `ScalarTrendReport{ points: Vec<ScalarTrendPoint>, delta, direction }` ✓ | `trend` |
| 6 | **MUTATION-RESULT** | a planned/applied edit set with dry-run. `Editor` recipes → `PlannedEdit`; undo/redo; rename | `edit` |
| 7 | **DIAGNOSTIC-REPORT** | findings/violations with severity + pass/fail. `rules run` findings, `analyze health`, `budget check`, `ratchet check`, `ci` | `check` |
| 8 | **CONFIG-CRUD** | add/update/show/remove of stored config + the resource it manages. `budget` CRUD, `ratchet` CRUD, `rules add/enable/disable`, `config`, `daemon`, `grammars` | `config` (and sibling resource verbs) |

Two structural shapes recur but are *infrastructure*, not analysis, so they keep their
own verbs rather than being forced into the eight:

- **AGGREGATE/DASHBOARD** (`analyze summary`, `analyze all`, `structure stats`) — a
  bag of mixed scalars and sub-reports. This is the shape that *resists* the frame
  (see §5); it is a deliberate composite.
- **ACTION/STATUS** (`structure rebuild`, `daemon start`, `serve`, `update`,
  `generate`) — side-effecting commands whose "return value" is a status, not data.

### Index-first axis is orthogonal

Whether a command needs the facts index is **independent of its shape**. A RANKED-LIST
can be single-file (`rank complexity` on one path) or index-requiring (`rank imports`);
a GRAPH is always index-requiring; an ENTITY-VIEW spans both (`view` single-file,
`view dependents` index). So index-dependency is *not* a taxonomy axis — it is a
per-command precondition (and a hard-constraint about non-zero exit, per audit T1-1).
The shape verb stays the same regardless.

---

## 2. Mapping table for the contested cross-section

Grounded in each command's actual return struct.

| Today | Return shape (evidence) | Proposed home |
|-------|-------------------------|---------------|
| `rank complexity/ceremony/length/uniqueness/call-complexity/duplicates/duplicate-types/fragments` | RANKED-LIST `Vec<Entry>` ✓ | `rank *` (stays) |
| `rank imports/surface/depth-map/layering/module-health/density` | RANKED-LIST ✓ (`DepthMapReport.entries`) | `rank *` (stays) |
| `rank files/hotspots/coupling/ownership/contributors/test-ratio/test-gaps` | RANKED-LIST ✓ | `rank *` (stays) |
| `rank size` | **TREE** — "hierarchical lines-of-code breakdown (ncdu-style tree view)" | **`tree size`** (moves out of rank) |
| `rank budget` | **breakdown** — `LineBudgetReport{ categories: Vec, modules: Vec }` ✓; two grouped ranked tables, not one list | **`rank purposes`** (stays in rank, renamed; resolves H-2) |
| `analyze architecture` | **GRAPH** — `ArchitectureReport{ cross_imports, hub_modules, coupling_hotspots, layer_flows, orphan_modules }` ✓ | **`graph architecture`** |
| `view graph` | **GRAPH** — `GraphReport{ sccs, diamonds, bridges, chains, dead_nodes }` ✓ | **`graph topology`** (moves out of view) |
| `analyze coupling-clusters` | **GRAPH** — connected components / BFS clustering of co-change | **`graph coupling`** |
| `rank coupling` | RANKED-LIST — file *pairs* ranked by co-change count | `rank coupling` (stays — different shape from the cluster graph) |
| `cfg cfg` | GRAPH (single function) — `CfgReport{ output, block_count, edge_count }` ✓ | **`graph cfg`** (collapses double-wrap; resolves H-1) |
| `view` / `view chunk` / `view list` / `view references` / `view referenced-by` / `view trace` / `view blame` / `view history` | ENTITY-VIEW (`ViewReport`, `TraceReport{trace}` ✓) — one target | `view *` (stays) |
| `view dependents` / `view import-path` | ENTITY-VIEW of one target's edges (a path/closure *for a node*, not the whole graph) | `view *` (stays — see §5 ambiguity note) |
| `structure stats` | AGGREGATE (`StructureStatsReport`) | `structure stats` (infra) |
| `structure files` | FLAT-LIST → treat as RANKED with no sort (`StructureFilesReport`) | could be `rank files` peer; kept under `structure` as index introspection (infra) |
| `structure rebuild/packages/query/test-fixtures` | ACTION/STATUS | `structure *` (infra, stays) |
| `syntax ast` | TREE (CST nodes) | `tree ast` *or* keep `syntax ast` (infra; see §5) |
| `syntax query` | RANKED/FLAT list of matches | `syntax query` (infra) |
| `syntax node-types` | FLAT-LIST of node kinds | `syntax node-types` (infra) |
| `trend multi/complexity/length/density/test-ratio` | TIME-SERIES ✓ | `trend *` (stays) |
| `budget measure` | breakdown (one measurement) | `rank purposes` peer? No — kept: see below |
| `budget add/check/update/show/remove` | CONFIG-CRUD + `check`→DIAGNOSTIC | split: CRUD stays `budget`; `budget check` → **`check budget`** |
| `ratchet measure/add/check/update/show/remove` | CONFIG-CRUD + `check`→DIAGNOSTIC | same: CRUD stays `ratchet`; `ratchet check` → **`check ratchet`** |
| `analyze health/summary/all` | AGGREGATE/DIAGNOSTIC | `check health` / `view summary` (see §5) |
| `analyze liveness/effects/exceptions/security/docs` | DIAGNOSTIC-REPORT (findings) | **`check *`** |
| `analyze skeleton-diff` | diff (a delta report) | `view diff`/`check diff` — borderline (see §5) |
| `rules run` | DIAGNOSTIC-REPORT (findings) | **`check rules`** |
| `rules list/show/tags/add/update/remove/enable/disable/setup/compile/test` | CONFIG-CRUD + meta | `rules *` (stays — resource CRUD) |

---

## 3. Resolving the four specific issues via the shape principle

**(1) analyze/rank boundary undefined.** The frame draws the line with zero
judgement calls: **does the command return a `Vec<Entry>` sorted by a score? → `rank`.
Does it return findings with severity / pass-fail? → `check`. Does it return
nodes-and-edges? → `graph`.** The migrated commands (complexity/length/duplicates/
ceremony/size/density) moved to `rank` *precisely because they return ranked lists* —
the frame predicts the migration that already happened. The residual `analyze` bucket
splits cleanly: its findings-shaped members (liveness, effects, exceptions, security,
docs, health) become `check`; its graph-shaped members (architecture, coupling-clusters)
become `graph`. `analyze` as a verb **disappears** — it was never a shape, it was a
topic. (Exception flagged honestly: `rank size` is a TREE, not a list — it moves to
`tree size`, which is the frame correctly *catching a miscategorization that exists
today*.)

**(2) `rank budget` vs `budget` collision.** Two different shapes, so two different
verbs — the collision was a symptom of shape-blind naming. `rank budget` returns
`LineBudgetReport` (a grouped breakdown of line counts by purpose) → it is a ranked/
grouped analysis → **`rank purposes`**. The `budget` service manages stored PR diff-size
limits (CONFIG-CRUD) → its mutating verbs stay under the resource noun `budget`, and its
one DIAGNOSTIC verb (`budget check`) surfaces under **`check budget`**. The word
"budget" now appears only where the *resource* budget lives; the line-breakdown analysis
no longer borrows it.

**(3) Near-duplicates.**
- `analyze architecture` vs `view graph`: both are GRAPH shape → both live under
  **`graph`** (`graph architecture` = coupling/hubs/layers view; `graph topology` =
  SCC/bridge/centrality view). Co-locating them under one verb makes the overlap
  *visible* (audit M-1 was that neither help mentioned the other) and forces the
  question of whether they should merge — which the frame makes a one-verb decision
  instead of a cross-service one.
- `analyze coupling-clusters` vs `rank coupling`: **different shapes, correctly
  separated.** `coupling-clusters` returns connected-component groups (GRAPH) →
  `graph coupling`; `rank coupling` returns ranked file *pairs* (RANKED-LIST) →
  `rank coupling`. Same source data, genuinely different output shape, so the frame
  *keeps them apart* and the names now signal the difference (graph of clusters vs
  ranked list of pairs).

**(4) `cfg cfg` double-wrap.** `CfgReport` is a graph (blocks + edges, rendered as a
Mermaid flowchart) about one function → it is a single command of shape GRAPH →
**`graph cfg`**. The redundant service wrapper vanishes because the verb is now the
shape (`graph`), not a re-statement of the noun (`cfg`).

---

## 4. Migration cost, blast radius, sequencing

**Net verb changes** (consolidating ~30 services toward shape verbs):
- New/retained shape verbs: `rank`, `graph`, `view`, `tree`, `trend`, `edit`, `check`.
- `analyze` **retired** (split into `check` + `graph`).
- Resource CRUD verbs retained as-is (`budget`, `ratchet`, `rules`, `config`, `daemon`,
  `grammars`, `package`, `sessions`, `kg`) — these are CONFIG-CRUD/resource surfaces and
  the frame does not touch them. They were never the source of the bugs.

**Blast radius (what breaks for users / docs):**
- `analyze *` → `check *` / `graph *`: ~13 command paths. Highest blast radius; `analyze`
  is heavily referenced in guides (already stale per H-4/H-5) and README/LLMS.md.
- `view graph` → `graph topology`, `cfg cfg` → `graph cfg`: 2 paths, low usage.
- `rank size` → `tree size`, `rank budget` → `rank purposes`: 2 paths.
- `budget check`/`ratchet check` → `check budget`/`check ratchet`: 2 paths; splits a
  service so the `check` verb aggregates all pass/fail commands.
- Internal: report structs already live in `commands/analyze/` regardless of mount point,
  so **moving a command between verbs is a routing change, not a struct move** — cheap.
  The `#[cli]` mounts change; the structs/`OutputFormatter`s do not.

**Pre-1.0 + "retire, don't deprecate":** clean break, no aliases. One flag-day commit
per verb consolidation, each landing with its `docs/cli/`, `README.md`, `LLMS.md`,
`guide`-body, and snapshot-test update in the same commit (CLAUDE.md sync rule). The
guide test proposed in audit T1-6 should land *first* so the renames can't reintroduce
stale references.

**Incremental vs flag-day:** verb-by-verb is feasible because each shape verb is
independent. Recommended order (lowest blast first to validate the frame, then the big
one): (a) `graph` (fold in `cfg`, `view graph`, `analyze architecture`,
`coupling-clusters`); (b) `tree size`, `rank purposes`; (c) `check` (fold in
`rules run`, the findings-shaped `analyze *`, `budget/ratchet check`); (d) retire the now
empty `analyze` verb. Each step is a single flag-day for *that verb*.

---

## 5. Honest trade-offs

**Where the data-shape frame is strong:**
- **Predictability for API consumers.** This is the frame's home turf and it directly
  serves the project's stated identity. An agent that knows it wants "a ranked list" goes
  to `rank`; "a graph" → `graph`; "pass/fail" → `check`. The `--json` shape is inferable
  from the verb. The verb *is* the schema family.
- **The analyze/rank boundary becomes a mechanical test**, not a judgement call — which
  is exactly the failure the audit documented (commands migrated with no written
  rationale, breaking guides). Shape is checkable in code review: "does this return a
  sorted `Vec<Entry>`? then it's `rank`."
- **It explains the migration that already happened** (quality metrics → `rank`) and
  **catches a latent miscategorization** (`rank size` is a tree, not a list).
- **Collisions dissolve structurally:** `rank budget` vs `budget` was a shape-blind
  name reuse; once the verb carries the shape, the same word can't denote two shapes.

**Where it is thin — the human-usability risk (the decisive weakness):**

> **`graph` and `check` are shape names, not task names — and a user does not think in
> shapes, they think in questions.** "Show me the control-flow of this function," "find
> circular imports," and "what are my most coupled modules" are three different *tasks*
> that the frame scatters across `graph cfg`, `graph topology`, and `rank coupling`
> purely because the first two return adjacency data and the third returns a sorted
> list. A user hunting "coupling" must know the *shape* of the answer they want before
> they can pick the verb — which inverts how discovery works. This is the frame's core
> liability for THIS problem: **it optimizes the projection for the machine consumer at
> the cost of the human's mental model**, and CLAUDE.md is explicit that a taxonomy
> "correct by data-model but unusable by humans is a failure."

Concrete soft spots:

- **Shapes that don't map to intuitive verbs.** `check` (DIAGNOSTIC) is fine — users
  say "check my code." `rank` is fine — "rank by complexity." But `graph` as a verb is
  awkward ("graph cfg"?) and `tree` even more so ("tree size"). English wants
  *nouns* here (`cfg`, `architecture`) or *task verbs* (`analyze`), not shape labels.
- **Ambiguous shapes force arbitrary calls.** `view dependents` returns a closure of
  edges *for one node* — is that ENTITY-VIEW (a fact about one symbol) or GRAPH (edges)?
  I assigned it to `view` by "it's about one target," but a different reader would say
  `graph`. `skeleton-diff` (a delta) and `analyze summary`/`all` (a dashboard of mixed
  sub-reports) have **no clean shape** — they are deliberate composites, and the frame
  has to special-case them (AGGREGATE/ACTION verbs) rather than place them. Every
  special case is an admission the single axis doesn't cover the space.
- **It cross-cuts subject affinity users rely on.** Today a user exploring imports finds
  `rank imports`, `view dependents`, `view graph`, `analyze architecture` somewhat near
  each other by topic. The shape frame *deliberately* scatters them across three verbs.
  Good for "I know the shape I want," bad for "I'm exploring imports."
- **`syntax ast` (TREE) vs keeping it under `syntax`.** Pulling it to `tree ast` to be
  consistent with `tree size` would split the syntax-inspection trio (`ast`/`query`/
  `node-types`) that users treat as one toolbox. The frame says split; usability says
  don't. I left it under `syntax` and flagged the inconsistency — which is itself
  evidence the pure frame has to be compromised to stay usable.

**Mitigation if this candidate were chosen:** keep the resource/infra verbs as task/noun
homes (don't shape-ify `rules`, `config`, `daemon`, `syntax`, `package`), apply the shape
frame only to the *analysis* surface where the analyze/rank bug actually lives, and add
topic-based aliases or a `normalize <topic>` discovery index (e.g. `normalize help
imports` listing every import-related command across verbs) to recover the discoverability
the frame sacrifices. That hybrid keeps the frame's strongest win (the mechanical
analyze/rank/check/graph boundary) while bounding its worst usability cost.
