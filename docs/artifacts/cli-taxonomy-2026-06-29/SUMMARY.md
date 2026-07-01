# CLI Taxonomy Redesign — 2026-06-29

Design artifacts for the normalize CLI command-taxonomy retree. The redesign exists
because commands silently migrated between services (`analyze`→`rank`) and broke guides
(H-4/H-5) with no objective rule governing placement. Source inventory:
`docs/artifacts/cli-audit-2026-06-29/05-command-structure.md`.

## The decision

**Full inversion (2026-06-30, supersedes the shape retree).** Primary membership axis =
**crate ownership**: push the `#[cli]` service DOWN into the compute crate so the
top-level verb *is* the owning crate (per CLAUDE.md's "a crate that owns a subcommand
includes its own `#[cli]` service"). Seam-corrected scope confirmed by `seam-evaluation.md`.

**Architecture extractions (decided, independent of verb taxonomy):**
- `normalize-git` — extract `git_utils.rs` low-level gix read ops into a new crate; migrate
  all 6+ dependents (normalize-budget, normalize-ratchet, normalize-semantic, normalize-native-
  rules, normalize-facts, main crate). Justified by verbatim multi-dependent duplication today.
- `normalize-git-history` — extract git-history analysis (hotspots/coupling/ownership/activity)
  into a standalone crate. Justified by standalone-tool category (code-maat/git-of-theseus).
  Becomes the `history` verb.
- Fold cyclomatic-complexity tree-walking into `normalize-facts::extract` (dedup, not new crate).

**CLI inversion (reachable crate-owned moves):** `graph` (normalize-graph), `architecture`
(normalize-architecture), `similarity` (normalize-code-similarity), `structure` (normalize-facts
— mount real `FactsCliService` + absorb dataflow trio), `filter` (normalize-filter), `search`
(normalize-semantic — wire the orphan), `history` (normalize-git-history). Blast radius ~20
commands (~12% of ~165). New crates: 2. Commands renamed: ~5.

**Metric core stays A1 (confirmed by seam evaluation):** complexity, length, ceremony, density,
imports, surface, size, files, test-ratio, test-gaps have no coherent compute crate — two
disjoint dependency groups (AST-group vs index-bound), not a domain. No `normalize-metrics`-AST
crate. The metric/git-history/dashboard residual stays under `rank`/`trend`/`analyze` (main-
crate verbs). Add a `RankEntry`-based CI lint to hold against future drift. See `seam-evaluation.md`.

**Cross-cutting:** `overview` (thin main-crate composition for health/summary/all), `trend`
(stays main-crate cross-cutting). See `00-inversion-plan.md` FINAL SCOPE section.

## Contents

**Authoritative plan (implement from this):**
- `00-inversion-plan.md` — command→owning-crate mapping (ground truth), the inverted verb
  set, struct/wiring moves per crate, bug-cluster fixes (structure/filter/syntax-rules/
  semantic/graph-gating), batched execution plan + blast radius, ownership CI lint, and
  open naming questions. The §0 STOP-flag on the ownerless metric residual is load-bearing.

**Superseded plan (retained for its candidate/judge synthesis):**
- `00-retree-plan.md` — the output-shape retree (rank/view/check/trend/overview). Marked
  SUPERSEDED; its contested-command analysis still informs the residual's editorial homes.

**Candidate designs (four decorrelated frames, design-only):**
- `candidate-A-subtract.md` — minimize: 4 shape verbs (view/rank/check/edit) + admin tier.
- `candidate-B-data-shape.md` — organize by output data-shape (8 shape verbs).
- `candidate-C-user-task.md` — organize by user task/workflow + objective I/O procedure.
- `candidate-D-input-scope.md` — organize by input scope/prerequisite (rejected).

**Adversarial judges (three lenses):**
- `judge-objectivity.md` — re-accretion resistance & lint-enforceability. Verified
  `RankEntry` is the only real, lint-catchable signal; D's prerequisite signal is buried/
  silently-degrading. `analyze` does not survive as a verb.
- `judge-usability.md` — discoverability/navigability. Human-guessable verbs + two-level
  topic structure; reject `graph`/`tree` as top-level verbs; reject D's `index` grab-bag.
- `judge-migration.md` — migration cost / API-first / merge legality. Flagged the
  enum-wrap risks; established that one-release transitional aliases are permitted.

**Seam evaluation (ground truth for architecture extractions):**
- `seam-evaluation.md` — investigates whether metric core + git-history compute warrant
  extraction. Verdict: metric core FAILS (not a coherent domain; two disjoint dep groups;
  collides with existing normalize-metrics). Git cluster: `normalize-git` PASSES on
  multi-dependent duplication criterion (a); `normalize-git-history` PASSES on standalone-
  useful criterion (b). These findings drive the final scope in `00-inversion-plan.md`.

**Crate-ownership investigations (ground the inversion plan):**
- `crate-cli-surface-census.md` — the 47-crate A/B/C census: which crates have a `#[cli]`
  service, which are mounted, which carry partial CLI surface (graph's ungated
  `OutputFormatter`, the semantic orphan).
- `crate-ownership-map.md` — current mount structure; proves analyze/rank/trend are one
  main-crate body over pure-library compute crates (no crate boundary backs the 3-way split).

**normalize-graph architecture investigations (B2/B3 prerequisite):**
- `graph-crate-reality.md` — reality check: the crate's "pure primitives" claim is false;
  `GraphTarget`/`DependentEntry.has_tests`/`BlastRadius`/`OutputFormatter` impls are all
  normalize-specific; domain is split across two crates with concrete duplication
  (`find_longest_chains` independently reimplemented in normalize-architecture).
- `graph-crate-justification.md` — justification audit: petgraph absent from workspace and a
  poor fit for adjacency-list API; algorithms String-pinned but not semantically entangled
  (genericization is mechanical); exactly 2 workspace consumers, one thin; ~60% generic
  algorithm bodies vs ~40% normalize-specific; several algorithms (diamond detection,
  longest-path-with-suffix-dedup, directional bridge) have no petgraph equivalent.
- `DECISION-graph-crate.md` — decision record: separate the generic and normalize-flavored
  halves; generic half → datatype-agnostic functions (bring-your-own-representation, zero
  normalize deps); normalize-flavored half → dependency-analysis callers + normalize-facts;
  kill the `find_longest_chains` duplication. DECIDED IN PRINCIPLE, DEFERRED — not to
  execute mid-taxonomy migration. B2/B3 are blocked on resolving this boundary.

## Synthesis

**Final seam-corrected scope (current):** 2 new crates (normalize-git, normalize-git-history)
+ crate-owned verb moves for `graph`, `architecture`, `similarity`, `structure` (facts),
`filter`, `search`, `history`. Metric core confirmed A1 by seam evidence — stays in main
crate. Blast radius: ~20 commands re-pathed (~12%), 2 new crates, 6+ dependents migrated to
normalize-git. Batched execution plan in `00-inversion-plan.md` FINAL SCOPE section (B0–B12).

**Superseded shape synthesis (retained):** B's mechanical shape *rule* + C's human verb
*names* + a topic second level. Final shape verbs were `rank`/`view`/`check`/`trend`/
`overview`/`edit`. Its contested-command analysis still informs where the inversion's
ownerless residual could land editorially.
