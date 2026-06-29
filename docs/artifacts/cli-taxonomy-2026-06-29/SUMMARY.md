# CLI Taxonomy Redesign — 2026-06-29

Design artifacts for the normalize CLI command-taxonomy retree. The redesign exists
because commands silently migrated between services (`analyze`→`rank`) and broke guides
(H-4/H-5) with no objective rule governing placement. Source inventory:
`docs/artifacts/cli-audit-2026-06-29/05-command-structure.md`.

## The decision

**Full retree.** Primary membership axis = output **shape** (lint-enforceable via the
`RankEntry` trait); verb **names** stay human-guessable; structure is two-level
(verb + topic); `analyze` is dissolved; no enum-wrapping; one-release transitional
aliases allowed. See `00-retree-plan.md`.

## Contents

**Authoritative plan (implement from this):**
- `00-retree-plan.md` — final verb set, two-level topic structure, complete command→
  new-home mapping, CI lint spec, transitional alias plan, guide regression test,
  migration execution plan, and open naming questions.

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

## Synthesis

The plan = B's mechanical shape *rule* (the drift-proof tiebreak, lint-enforced) +
C's human-guessable verb *names* + an enforced topic second level inside the populous
verbs. Final verbs: `rank`, `view`, `check`, `trend`, `overview` (name TBD), `edit`,
plus the kept specialist/admin domains. Blast radius ~22 commands (~13%).
