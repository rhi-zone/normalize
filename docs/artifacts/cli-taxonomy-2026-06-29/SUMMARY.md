# CLI Taxonomy Redesign — 2026-06-29

Design artifacts for the normalize CLI command-taxonomy retree. The redesign exists
because commands silently migrated between services (`analyze`→`rank`) and broke guides
(H-4/H-5) with no objective rule governing placement. Source inventory:
`docs/artifacts/cli-audit-2026-06-29/05-command-structure.md`.

## The decision

**Full inversion (2026-06-30, supersedes the shape retree).** Primary membership axis =
**crate ownership**: push the `#[cli]` service DOWN into the compute crate so the
top-level verb *is* the owning crate (per CLAUDE.md's "a crate that owns a subcommand
includes its own `#[cli]` service"). Central honest finding: inversion cleanly extracts
the genuinely crate-owned families (`graph`, `architecture`, `similarity`, dataflow→
`structure`) and fixes the mounting bugs, but the **metric core of `rank` + git-history
cluster + dashboards + `trend` have no compute crate** (compute lives in the main crate),
so inversion does NOT dissolve them — they stay main-crate verbs unless a prior
compute-extraction phase runs. See `00-inversion-plan.md`.

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

**Crate-ownership investigations (ground the inversion plan):**
- `crate-cli-surface-census.md` — the 47-crate A/B/C census: which crates have a `#[cli]`
  service, which are mounted, which carry partial CLI surface (graph's ungated
  `OutputFormatter`, the semantic orphan).
- `crate-ownership-map.md` — current mount structure; proves analyze/rank/trend are one
  main-crate body over pure-library compute crates (no crate boundary backs the 3-way split).

## Synthesis

**Inversion plan (current):** organize by crate ownership. Crate-owned verbs reachable now
= `graph`, `architecture`, `similarity`, `structure` (facts), `filter`, `search` (semantic)
+ kept `budget`/`cfg`/`kg`/`ratchet`/`rules`. Reachable blast radius ~18 commands (~11%).
The ownerless metric/git-history/dashboard residual stays main-crate (`rank`/`trend`) —
flagged, not forced. See `00-inversion-plan.md` §0.

**Superseded shape synthesis (retained):** B's mechanical shape *rule* + C's human verb
*names* + a topic second level. Final shape verbs were `rank`/`view`/`check`/`trend`/
`overview`/`edit`. Its contested-command analysis still informs where the inversion's
ownerless residual could land editorially.
