# Judge — Migration-Cost / API-First / Is-a-Retree-Warranted?

*Adversarial review of candidates A–D, 2026-06-29. Lens: migration cost realism,
API-first fidelity, and whether a 30–35% flag-day retree earns its keep. Not committed.*

The burden of proof is on the full retree. A 50-command rename is the expensive option;
it has to beat the cheapest thing that fixes the actual bugs, not the status quo.

---

## Attack 1 — The baseline nobody proposed: RULE + targeted fixes, no retree

### Construct the minimal option

1. **Adopt one objective placement RULE as forward-looking law + a CI lint.** Borrow
   Candidate C's decision procedure (rule 5 vs 7: *metric-sorted `Vec` → `rank`; unordered
   scope judgment → `analyze`*) or Candidate B's shape test (*does the Report's load-bearing
   field sort by a score?*). Add a lint: a `#[cli]` method mounted under `rank` must return a
   type whose primary field is a sorted collection; under `check`, a verdict; etc. CI fails on
   violation.
2. **Rename `rank budget` → `rank purposes`** (T2-6). One command.
3. **Collapse `cfg cfg` → `cfg <path>`** (T2-7). Remove a redundant service wrapper; the leaf
   spelling barely changes.
4. **The two near-dups (T3-4):** add see-also cross-references now (the audit's own cheap
   fix). Merge `analyze architecture`↔`view graph` *only* if someone does the genuine
   struct-union (see Attack 3) — otherwise leave them cross-referenced.
5. **Rename `edit history` → `edit log`** (T2-10) — adjacent, cheap, kills the `view history`
   confusion.
6. Fix the broken guides (T1-6) + land the guide-snapshot test. (This session already did the
   guide fix.)

### How much of the problem does this solve?

**The drift bug is the whole problem, and the rule kills it.** The audit's root cause is not
"the tree is ugly" — it is that the analyze/rank boundary was a *vibe*, so commands migrated
between two homes and guides rotted (H-4/H-5). The decisive fact, stated by Candidate C
itself: applying the rule to today's tree requires **zero command moves** — "none currently
need to move… the rule just names it so the next feature can't drift." So the entire
structural win is available **without renaming anything**.

A lint converts the drift bug from *will-recur* to *caught-at-CI*. That is functionally what
the retrees claim as their headline ("impossible by construction"). The only delta between
the baseline and a full retree, *on the drift axis*, is impossible-by-construction vs
caught-at-CI — and a lint that fails the build is, for engineering purposes, the same
guarantee.

### What does the full retree buy that the minimal option doesn't?

- **Verb-set aesthetic coherence.** A cleaner mental model (4–8 shape/task/scope verbs). Real,
  but it is taste, not a bug fix.
- **Nothing on drift-prevention** that the lint doesn't already give.
- And it buys it at the cost of re-breaking every guide, doc, snapshot, and CI invocation —
  the exact wound this session just healed (H-4/H-5). The retree re-craters the road before the
  guide-test that would catch the re-crater even exists.

**Verdict: the minimal baseline dominates.** It captures ~90% of the value (kills the drift
class, fixes all four concrete issues) at ~5% of the blast radius. The full retree clears the
burden of proof only if verb-set elegance is worth re-breaking the just-fixed guide layer
pre-1.0 — and it isn't, this month.

---

## Attack 2 — Flag-day realism + "retire, don't deprecate" mis-application

All four candidates assert that "retire, don't deprecate" forbids transitional aliases and
therefore *forces* a single flag-day break of the two highest-traffic services + every CI
invocation + all guides/docs/snapshots. **This is a mis-read.**

The CLAUDE.md rule is: *"Retire, don't deprecate… Remove backward-compat aliases rather than
carry them."* That governs **permanent compat cruft** — the habit of carrying an old spelling
indefinitely so you never have to break anyone. It says nothing against a **transitional**
alias that exists for exactly one release as a migration tool and is then removed. The two are
different things: deprecation is a forever-tax; a one-release hidden alias is scaffolding you
take down.

**The project's own audit already endorses exactly this.** T2-3's verdict on `--base-ref`:
*"Optionally accept `--base-ref` as a hidden alias for one release if any CI configs in the
wild used it."* The repo treats a one-release transitional alias as fully compatible with
retire-don't-deprecate. The candidates over-read a ban-on-permanent-cruft into a
ban-on-migration-tooling.

The companion rule actually cuts the *other* way: *"Finish migrations before building on top;
fence what you can't finish."* The danger it names is the *half-finished* state where old
patterns dominate by count and get copied forward. A completed-and-removed one-release aliased
migration finishes the migration just as much as a flag-day does. What the rule forbids is the
indefinite half-state, not the existence of scaffolding during a completed migration.

**So: an incremental/aliased path is legitimate.** Flag-day is not forced by the rules; it is
the candidates' preference dressed as a constraint.

Is flag-day *survivable* pre-1.0? Yes (few external users). Is it *justified* right now? No —
it is gratuitous risk for a tool that just spent effort repairing guides broken by the last
rename, and the guide-snapshot test (T1-6) that would catch a re-break **does not exist yet**.
Sequencing rule regardless of scope: **land the guide-snapshot test first, then touch any
command name.** The minimal baseline barely moves anything, so it needs almost no aliases and
carries almost no flag-day risk either way — which is another point in its favor.

---

## Attack 3 — Enum-wrap / data-distortion check on every proposed merge

CLAUDE.md bans "unifying commands by wrapping N report types in an enum" and forbids distorting
data shape for CLI aesthetics. Every merge, judged:

| Merge | Candidate | Verdict |
|-------|-----------|---------|
| `rank coupling` + `analyze coupling-clusters` → `rank coupling --group pairs\|clusters` | **A** | **VIOLATION.** `Vec<Pair>` vs `Vec<Cluster>` are two return types behind a flag = the banned enum-wrap. A *self-flags this* ("exactly the anti-pattern CLAUDE.md forbids") and retreats to two subcommands. Honest, but the merge as written is illegal. |
| `analyze health/all/summary` → `rank summary` | **A** | **Data-shape distortion.** Dashboards are an AGGREGATE composite, not `Vec<Scored<T>>`. Filing under a list verb is a category fudge (A admits "awkward"). |
| `trend *` → `rank … --over-history` | **A** | **Shape distortion / borderline enum-wrap.** TimeSeries (`Vec<Point>` + delta/direction) ≠ RankedList; folding it behind a flag unions unlike shapes. |
| `coupling-clusters` + `rank coupling` → `history coupling --cluster` / `--by pair\|cluster` | **D** | **VIOLATION, unacknowledged.** Identical pairs-vs-clusters union that A flagged — but D presents it as a clean "one git pass, two aggregations, one command" with no caveat. This is the banned enum-wrap, undeclared. |
| `analyze architecture` + `view graph` → one report (A's `view graph`, C's `analyze architecture`, D's `index graph`) | **A, C, D** | **Conditional.** OK *only as a genuine struct-union* — one whole-graph-pathology report carrying sccs + hubs + coupling-pairs + centrality together (they are the same domain). It is an enum-wrap if implemented as flag-selects-one-of-two-structs. All three frame it as a union, so it's legal in principle — but it is real struct-merge work, not a free rename, and the minimal baseline can instead just cross-reference the pair (T3-4's cheaper option) and skip the merge entirely. |
| `analyze architecture`/`view graph` co-located under `graph` as **separate** subcommands (`graph architecture`, `graph topology`) | **B** | **No violation.** Co-location under a mount namespace ≠ union type. |
| `coupling-clusters` (→`graph coupling`) vs `rank coupling` kept **separate by shape** | **B, C** | **Correct handling.** Same source data, different output shape, kept apart. This is what the rule wants. |

**Summary:** Candidate **A** carries three data-distorting merges (one a self-flagged
enum-wrap, two shape-fudges). Candidate **D** carries one *unacknowledged* enum-wrap
(coupling). Candidate **B** proposes **no** violating merges — it relocates and co-locates but
never unions report types; it is the cleanest on API-first. Candidate **C** has only the
architecture/view-graph struct-union, handled correctly. The architecture↔view-graph merge is
the one merge all three retrees converge on and the one the *minimal baseline can avoid
outright* by cross-referencing instead.

---

## Attack 4 — Cost ranking (command-move count + blast radius + re-break risk)

| Cand. | Path changes (own §4) | Blast radius / risk | Value-per-breakage |
|-------|----------------------|---------------------|--------------------|
| **C** | analyze/rank leaves **don't move** (rule names existing membership); moves = `cfg cfg`, `rank budget`→`composition`, budget/ratchet→`check`, `ci`, `edit history`→`log`, the architecture merge, + a large-but-low-risk `manage` umbrella over infra | LOW-MODERATE. Keeps the high-traffic analyze/rank paths put → **does not re-break the just-fixed guides on those paths.** Most churn is the uncontentious infra→`manage` re-mount nobody navigates by guessing. | **Best.** It is essentially "minimal baseline + a manage umbrella + a few renames." Its own text concedes the rule needs zero analyze/rank moves — the seed of the dominating baseline. |
| **B** | ~20: `analyze`(13)→`check`/`graph`, `view graph`→`graph topology`, `cfg cfg`→`graph cfg`, `rank size`→`tree size`, `rank budget`→`purposes`, budget/ratchet check→`check` | MODERATE. Re-breaks `analyze` paths. Introduces shape-verbs (`graph`, `tree`) that B *itself* names as the decisive human-usability weakness ("a user thinks in questions, not shapes"). | Second. API-first-faithful, no violating merges, but pays a usability tax and still re-craters `analyze`. |
| **A** | ~45–50 (~30%): `analyze` dissolves entirely, `rank` absorbs ~9, budget/ratchet→`check` (~12), trend/cfg/syntax/edit moves | HIGH. Breaks the two highest-traffic services + every CI invocation. | Third. High churn *plus* three data-distorting merges it must walk back; `rank` becomes a 30-leaf grab-bag (intra-rank categories = the original problem relocated one level down). |
| **D** | ~55–60 (~35%): nearly every verb changes; `rank`/`analyze`/`trend`/`view` reshuffled into `index`/`history`/`fleet` | HIGHEST. Most invasive of all four. | Worst. The `index` verb becomes a 25-command grab-bag (D's own admitted fatal weakness — "the new `analyze`, just renamed"), per-file metrics split from index metrics on an implementation detail, and an unacknowledged enum-wrap. |

**Cost (low→high): C ≈ B < A < D. Value-per-unit-breakage (best→worst): C > B > A > D.**

But all four are dominated by the off-list minimal baseline, which is strictly cheaper than C
and delivers the same drift fix.

---

## Bottom line — how much to actually change

**Do the minimal baseline. Reject the verb-set overhauls.** Concretely:

1. **Land the guide-snapshot test (T1-6) FIRST** — before any rename — so a re-break of the
   H-4/H-5 class is caught.
2. **Adopt Candidate C's decision procedure (or B's shape test) as the placement law, plus a
   CI lint** mapping `#[cli]` mount path → return-type shape. **Zero command moves required** —
   this is the entire structural win.
3. **`rank budget` → `rank purposes`** (T2-6); **collapse `cfg cfg` → `cfg`** (T2-7);
   **`edit history` → `edit log`** (T2-10). Three cheap, isolated fixes.
4. **Two near-dups:** cross-reference now. Merge `analyze architecture`↔`view graph` **only**
   if someone does the genuine single-report struct-union — otherwise leave the see-also.
5. **Do not move the analyze/rank/view/trend leaves.** Re-evaluate a fuller, elegance-driven
   retree at the 1.0 boundary, if ever — and if so, do it as a completed one-release *aliased*
   migration (which the rules permit), not a guides-cratering flag-day.

The full retree must earn 40–55 renames. On the only axis that is a real bug — drift — the lint
matches it. On everything else it offers verb-set aesthetics, which do not justify
re-breaking the guide/doc/snapshot/CI layer the team just repaired. **It has not earned it.**
If forced to pick a retree anyway, **Candidate C** is the least-bad (lowest re-break risk,
no violating merges, and it is the closest to the minimal baseline); **Candidate D** is the
worst (highest churn, admitted grab-bag, unacknowledged enum-wrap).
