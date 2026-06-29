# Judge — Re-accretion Resistance & Rule Objectivity

*Adversarial evaluation, 2026-06-29. Not committed. Lens: the redesign exists because
commands silently migrated between services (analyze→rank) and broke guides (H-4/H-5)
because no objective rule governed placement. The only question here: **which candidate's
membership rule is genuinely objective AND lint-enforceable such that this CANNOT recur.***

## Evidence base (verified against source, not asserted)

Two code investigations grounded this judgment:

**Rank/shape signal (for A/B/C):**
- A marker trait `RankEntry` **exists** — `crates/normalize-analyze/src/ranked.rs:97`. But
  it is a **table-rendering trait** (`fn columns()`, `fn values()`), not a "sorted-by-metric"
  semantic marker. Implemented by `FunctionComplexity`, `CoupledPair`, `ModuleHealthEntry`.
- `Scored<E>` exists (`ranked.rs:248`) but is **not uniformly used**. Each rank command returns
  a **distinct** struct (`ComplexityReport`, `SizeReport`, `ModuleHealthReport`, `CouplingReport`,
  …) — there is **no common return wrapper**. A type-level "all rank methods return X" lint is
  impossible; a "primary `Vec<T>` has `T: RankEntry`" lint is possible.
- `rank size` → `SizeReport { tree: Vec<SizeNode> }` with `SizeNode { children: Vec<SizeNode> }`
  (`commands/analyze/size.rs:10`). It is a **tree**, and `SizeNode` does **NOT** impl `RankEntry`.
  A RankEntry-presence lint would correctly **flag `size` as misplaced under rank**.
- `rank module-health` → `ModuleHealthReport { modules: Vec<ModuleHealthEntry> }` sorted by score
  (`commands/analyze/module_health.rs:58`); `ModuleHealthEntry: RankEntry`. **It is a flat sorted
  Vec.** The "part-summary" intuition is wrong at the struct level — the compositeness is
  *intra-entry* (each row carries test_ratio/uniqueness/density/ceremony), not report-level. So
  module-health is a clean ranking, and the rank/analyze line is drawable positively:
  `analyze health/summary/all` carry **no** `Vec<T: RankEntry>` primary field; every rank command
  does.

**Prerequisite signal (for D) — the decisive finding:**
- The four prerequisite methods exist (`crates/normalize/src/index.rs`): `open_if_enabled` (opt),
  `ensure_ready` (strict), `require_import_graph` (strict), `ensure_ready_or_warn` (opt+warn).
- But the calls are **NOT uniformly detectable**:
  - `require_import_graph` is called at **service level** for `architecture`, `graph`,
    `dependents`, `import_path` — detectable.
  - **Git prerequisites are buried** inside command fns: `rank hotspots` → `parse_git_churn(root)?`
    inside `commands/analyze/hotspots.rs:318`, no signal at `service/rank.rs:444`. `rank coupling`,
    `analyze activity` same — and `git_activity_commits` **silently returns empty** on no-repo,
    so the command "succeeds" with empty output (no detectable prerequisite at all).
  - **Optional enrichers are indistinguishable from zero-prerequisite**: `view trace` and
    `analyze docs` use `index::open_if_enabled()` / `index::open().await.ok()` with a fallback
    parser. Nothing at the type or call-site level says "index".
  - Only `multi_repo` (`discover_repos(...)?` at service level) is reliably detectable.
- **Conclusion: D's central claim — "the compiler-visible dependency *is* the classification" —
  is false in the current codebase.** A D-lint would require hand-maintained per-command
  annotation, which re-creates the exact drift D claims to abolish.

---

## Straddler stress-test (clean = rule gives ONE deterministic answer; arbitrary = judgment call)

Straddlers: `module-health`, `view trace`, `repo-coupling`, `coupling-clusters` vs `rank coupling`,
`size`, `view history/blame`, `rules run --fix`.

| Straddler | A (shape, 4 verbs) | B (shape, 8 verbs) | C (task procedure) | D (prerequisite) |
|---|---|---|---|---|
| **module-health** | CLEAN → rank (Vec scored) | CLEAN → rank | CLEAN → rank (rule 5; flat Vec<RankEntry>, C's worry overstated) | CLEAN → index (ensure_ready) |
| **view trace** | CLEAN → view | CLEAN → view | CLEAN → view (rule 6; index at error layer) | **ARB** — opt enrich; monotonic rule advertises a prereq it doesn't need (D **admits** "forces a lie") |
| **repo-coupling** | **ARB** — unmapped; ranked pairs or graph? | ARB — not in table; by struct | **ARB** — rule 7→analyze, but ranked→rule 5; conflict | CLEAN → fleet (multi_repo detectable) |
| **coupling-clusters vs coupling** | **ARB** — A **admits** the risky merge (pairs vs clusters = 2 types, enum anti-pattern) | **CLEAN** — clusters=graph, pairs=rank; CoupledPair impl RankEntry, cluster doesn't | CLEAN — pairs→rank (5), clusters→analyze (7) | CLEAN placement (both→history); merge is a separate flag question |
| **size** | **ARB/WRONG** — left in rank; it's a tree, A didn't notice | **CLEAN** → tree (caught; SizeNode lacks RankEntry) | **ARB/WRONG** — rule 5 calls it "sorted Vec"; it's a tree, no tree verb to catch it | CLEAN → index (prereq ignores tree-ness) |
| **view history/blame** | CLEAN → view (target dominates) | **ARB** — it IS a time-series; B has `trend` but files it under view without resolving the clash | **ARB** — rule 4 (git-history axis) fires *before* rule 6 → would pull to `trend`; C keeps it in view (contradiction) | CLEAN → history (git) |
| **rules run --fix** | **ARB** — check by fiat; --fix mutates source = edit | **ARB** — run=check, --fix=edit; same cmd, two shapes by flag | **ARB** — rule 1 (mutate source)→edit fires for --fix; rule 3→check for plain | **ARB** — D **admits** mutation is an orthogonal unmodeled axis |
| **Clean / 7** | **3/7** | **4/7** | **3/7** | **5/7** |

**Reading the count carefully.** D resolves the *most* straddlers (5/7) — but only because its rule
**ignores shape entirely**, so every straddler that is ambiguous *about shape* (module-health, size,
repo-coupling) collapses to "what does it read." That is real cleanliness of the **rule** — but it is
human-applied cleanliness, because the signal the rule needs is not in the code (see lint section).
B's 4/7 are resolved by a signal that **is** in the code (`RankEntry`), and B's two unique wins
(`coupling-clusters` split, `size` catch) are the two cases the historical bug-class lives in.

---

## Lint-enforceability — is it real?

| Cand | Claim | Real? | Why |
|---|---|---|---|
| **A** | "Vec<Scored> → rank, nowhere else" | **Partial / inconsistent** | No uniform `Vec<Scored>` wrapper exists; relies on the same `RankEntry` signal as B but A **mis-applies it** (leaves tree-shaped `size` under rank — the very lint B proposes would flag A's own placement). |
| **B** | "method under `rank` returns a type whose primary `Vec<T>` has `T: RankEntry`" | **REAL (strongest)** | `RankEntry` is a genuine, machine-detectable trait. The lint **provably fires on a real current miscategorization** (`size`/`SizeNode` has no `RankEntry`). The rank↔check↔graph seam — exactly where H-4/H-5 happened — becomes a CI failure. (Graph/tree markers unverified but the bug-locus seam is covered.) |
| **C** | "method under `rank` returns a sorted collection; lint enforces it" | **Partial — real exactly where the bug is** | The `Vec<T: RankEntry>` half is detectable and *does* draw the rule-5-vs-7 (rank vs analyze) line — analyze health/summary/all have no such field. BUT "sorted **by a metric**" is runtime behaviour in the method body, not in the type; and C's *other* rules (verdict→check, git-axis→trend, mutate-state→manage) hit the **same buried/optional signal problem as D** — not cleanly detectable. |
| **D** | "the code-level prerequisite (`require_import_graph`/`git_utils`/`multi_repo`) *is* the classification" | **NOT REAL** | Verified false: git prereqs are buried in command fns and **silently degrade** (empty Vec, no error); optional enrichers (`open_if_enabled`, `open().await.ok()`) are indistinguishable from zero-prereq. Only import-graph and multi-repo are service-level detectable. A D-lint needs hand-maintained annotation → **re-creates the drift it set out to kill.** |

**Verdict on the lint axis:** B's lint is the only one that is both real *and* demonstrably catches a
live error today. C's is real *only* at the rank/analyze seam (which is, fairly, the seam that broke).
A's is real but A applies it inconsistently. D's is aspirational — contingent on a uniform-prerequisite
refactor that does not exist, and D cannot even classify its own optional-enrichment commands.

---

## Orthogonal-axes problem (shape / input-scope / mutation are 3 independent axes; each picks one)

| Cand | Primary axis | Demotes | Demotion damage |
|---|---|---|---|
| A | shape | scope (→ error layer), admin | **rank swells to ~30** → needs *intra-rank* sections (the original grouping problem, one level down); admin tier is an honest "not a query" grab-bag but A admits it doesn't reduce. |
| B | shape (granular) | scope (orthogonal), topic | **Least intra-verb grab-bag** — each verb stays shape-coherent; scatters topic-families (imports across rank/graph/view → discoverability cost) and quarantines AGGREGATE/ACTION as honest special-cases rather than a verb. |
| C | task (via I/O procedure) | shape (partial), scope (→ error layer) | `manage` is a self-admitted ~12-command grab-bag; `analyze` is a residual "else" bucket (see fork). |
| D | input-scope | **shape AND topic entirely** | **`index` becomes ~25 unrelated computations** — D admits it is "the new analyze, renamed"; the what-it-computes distinction returns as **untested help-text sections**, where a misgroup is *invisible* (no guide catches a cosmetic label). Mutation is wholly unmodeled. |

**Least damage: B.** Demoting topic scatters families but never forms a within-verb junk drawer of
unlike things — the property that *created* the analyze/rank mess. D's demotion of shape does the
opposite: it manufactures the biggest single grab-bag of the four and makes its sub-structure
untestable.

---

## The analyze-survives fork — does "unordered whole-scope report" hold up objectively?

**No.** It is a **residual category, not a positive shape.** C's procedure defines it as rule 7 =
"OTHERWISE" — by construction the bucket for anything that isn't mutation/verdict/trend/ranking/
single-target. A residual bucket is *exactly* the re-accretion vector: every future feature that
doesn't fit the other six rules lands here, unexamined.

The struct evidence shows the legitimate cut is **positive and detectable**: `analyze health`,
`summary`, `all` carry **no** `Vec<T: RankEntry>` primary field (they are composite dashboards),
whereas every `rank` command — including the contested `module-health` — does. So the honest category
is **"aggregate/dashboard," a small enumerable special-case (3 commands)**, which is precisely how
B treats it (quarantined as "resists the frame"), not a first-class verb. C's error is **promoting the
residual to a top-level verb and painting a procedure over it.** The rule-5-vs-rule-7 seam *is* the old
analyze/rank seam; C makes it more governed (good) but leaves it a live verb boundary resting partly on
an undetectable qualifier ("sorted *by a metric*", "primary payload"). For a true composite that also
contains a list, the call is still a judgment — C's own §6 names `module-health` as the soft case
(though, per the struct, it actually resolves to rank).

**Bottom line on the fork:** `analyze` should be **dissolved**, not retained. "Unordered report" is the
catch-all renamed. A/B/D are right to fold it away; B does so most honestly (aggregates become a named,
bounded special-case rather than either a verb (C) or a fudged `rank summary` (A) or a silent member of
a 25-command `index` (D)).

---

## Per-candidate verdicts

### Candidate A — SUBTRACT (shape, 4 verbs)
- **Objectivity: 3/7 straddlers clean.** Misclassifies `size` (tree under rank) by its own rule;
  unmapped on `repo-coupling`; self-admits the `coupling-clusters` merge is arbitrary.
- **Lint real? Partial & inconsistent** — relies on `RankEntry` but mis-applies it.
- **Re-accretion: moderate.** Dissolving analyze→rank kills the *specific* historical migration (path
  can't change within `rank`), but swelling `rank` to ~30 relocates the grouping into untested
  sections, and the rank↔check↔edit verb seams remain (rules --fix straddles).

### Candidate B — DATA-SHAPE (shape, 8 verbs)
- **Objectivity: 4/7 straddlers clean** — and its two unique wins (`size`→tree, `coupling-clusters`
  split from `coupling`) are exactly the bug-class cases. Weak on time-series straddler
  (`history/blame` under view vs trend) and the `rules --fix` flag-flips-shape problem.
- **Lint real? YES — strongest.** `RankEntry` is detectable and provably flags a live misplacement.
- **Re-accretion: strongest.** Keeps boundaries but makes each a machine-checked property; the seam
  that broke becomes a CI failure. Cost: more verb boundaries = more surface, and shape-keyed mounting
  means a deliberate struct refactor *should* trigger a re-mount — but that fires the lint loudly in
  CI, so it's caught, not silent.

### Candidate C — USER-TASK (procedure, keeps analyze)
- **Objectivity: 3/7 straddlers clean.** The added rules surface *more* ordering contradictions:
  rule 4 (git-history axis) before rule 6 (target) would pull `view history/blame` into `trend`,
  contradicting C's own placement; `size` mislabeled "sorted Vec."
- **Lint real? Partial** — real only at the rank/analyze seam (via `RankEntry`); the verdict/git/state
  rules hit the same undetectable-signal wall as D.
- **Re-accretion: weakest.** It **preserves the exact analyze/rank verb boundary that broke**, governed
  by a procedure whose decisive seam (rule 5 vs 7) rests partly on an undetectable qualifier, plus a
  residual `else→analyze` catch-all and a self-admitted `manage` grab-bag.

### Candidate D — INPUT-SCOPE (prerequisite)
- **Objectivity of the RULE: 5/7 straddlers clean** — highest, because ignoring shape sidesteps every
  shape-ambiguity straddler. But it self-admits failure on `view trace` (optional prereq → "forces a
  lie") and `rules --fix` (mutation orthogonal, unmodeled).
- **Lint real? NO.** This is the disqualifier. The prerequisite signal is buried, optional, or silently
  degrading for the *majority* of commands (verified). The "verb = compiler-visible dependency" premise
  is false today; enforcement would require hand annotation = the drift returns.
- **Re-accretion: high potential, unrealized.** Dissolving analyze+rank into `index` is the most
  thorough kill of the path-migration bug *in principle*, and "verb = f(prerequisite)" would be the most
  stable rule *if detectable*. But it isn't, and it ships a 25-command `index` grab-bag whose
  sub-structure is untested help-text. Strongest idea, weakest as-built objectivity.

---

## Ranking — re-accretion resistance (the actual question)

1. **B (Data-Shape)** — the only rule that is *both* objective *and* lint-enforceable *and* demonstrably
   catches a live miscategorization. The seam that broke becomes a build failure.
2. **D (Input-Scope)** — most objective *rule* and most thorough boundary-dissolution, but its
   enforceability is aspirational (signal not in the code) and it manufactures the largest grab-bag.
   Strong if paired with B's shape-lint *inside* `index`; self-defeating alone.
3. **A (Subtract)** — dissolves the specific analyze→rank migration (good) but applies its own shape
   signal inconsistently (size) and relocates grouping into a swollen, sectioned `rank`.
4. **C (User-Task)** — keeps the exact boundary that broke, partial lint, residual catch-all. Most
   usable, least drift-proof — and drift is the documented disease.

**One-line synthesis:** the objective, enforceable core is **shape via `RankEntry`-style markers**
(B's mechanism, which A/C lean on implicitly); the most *stable* axis would be **prerequisite** (D)
**if** prerequisites were hoisted to uniform, statically-detectable service-level calls. The winning
design is B's enforceable shape-lint as the membership rule, optionally with D's prerequisite axis used
*within* a verb for sub-grouping — but only after the prerequisite calls are refactored to be
detectable. `analyze` does not survive as a verb under any honest reading: "unordered report" is a
residual, not a shape.
