# CLI Taxonomy — Candidate A: SUBTRACT / MINIMIZE

Frame: fewest top-level verbs, fewest concepts. Find the single organizing axis
that makes ~30 services stop being arbitrary. If we could only keep a handful of
verbs, what are they, and does every command fall cleanly under exactly one?

---

## 1. The organizing principle + verb set

**Principle (one sentence):** Group commands by the *shape of the answer the API
returns* — the same axis API-first already forces on the data model — so the
verb is a synonym for the result type, not for a topic area.

This is the only axis that collapses the sprawl without inventing topics. Topic
grouping ("quality", "structure", "git history") is what *created* the
analyze/rank mess: the same metric can be filed under two topics, so it migrates,
so guides break. Result-shape is not a matter of taste — every service method
already returns exactly one of a tiny set of shapes. Read the shape, you know the
verb. There is no second home.

**The minimal core verb set (4):**

| Verb | Answer shape | "Ask yourself" |
|------|--------------|----------------|
| **view** | One designated artifact, structured (a file, symbol, AST, CFG, the dependency graph, blame, history, a trace). Single target in → one representation out. | "Am I inspecting *one* thing I named?" |
| **rank** | A population scored/ordered → `Vec<Scored<T>>` (or grouped list). Many units in → an ordered list out. | "Is the answer a list of units with numbers on them?" |
| **check** | A verdict against a policy → pass/fail + violations + exit code. | "Does this answer pass or fail?" |
| **edit** | A mutation of code → a diff/plan, applied or dry-run. | "Does this change the source?" |

Plus one **irreducible admin tier** (see §5 — this is where SUBTRACT is honestly
thin): tool plumbing that is not a query about the codebase at all — index
builds, daemon/serve, grammars, config, codegen, session/kg/context stores,
package metadata, guides. SUBTRACT's claim is *not* that everything fits in 4
verbs; it is that **every code-intelligence command does**, and the remainder is
administration that does not reduce and should be named as such rather than
smeared into a fake fifth verb.

Why not fewer? The absolute floor is 2 — *read* vs *write*. But collapsing all
reads into one verb just renames `normalize complexity` → `normalize read
complexity`: zero information gained, discoverability destroyed. The result-shape
split (view/rank/check) is the *minimal useful* partition of "read": it is the
coarsest grouping that still tells the user something. Below it you are relabeling
tree depth, not subtracting concepts.

---

## 2. Mapping table (cross-section, all contested commands covered)

| Today | Lands under | Notes |
|-------|-------------|-------|
| `view <target>` / chunk / list / blame / trace / history / references / referenced-by / dependents / import-path | **view** | unchanged — canonical single-artifact inspection |
| `view graph` | **view graph** | canonical home for "the dependency graph and its pathologies" |
| `analyze architecture` | **view graph** *(merged)* | same computation (cycles + hubs + coupling) → folds into `view graph`; retired as a separate command |
| `cfg cfg` | **view cfg** | control flow of one named function = single-artifact inspection; double-wrap dies |
| `syntax ast` / `query` / `node-types` | **view ast** / `view query` / `view node-types` | inspecting the parse of one file = view (syntax service dissolves) |
| `structure stats` / `files` / `query` | **view** (index-* read side) | read-only index inspection is single-artifact view |
| `structure rebuild` / `packages` | **admin** | mutates the index → admin (index build), not a query |
| `analyze complexity-ish, duplicates, duplicate-types, fragments, security, docs, liveness, effects, exceptions` | **rank** | all return scored/listed populations → rank |
| `rank complexity / ceremony / length / uniqueness / call-complexity / size / density / imports / surface / depth-map / layering / module-health / files / hotspots / ownership / contributors / test-ratio / test-gaps` | **rank** | unchanged |
| `rank coupling` + `analyze coupling-clusters` | **rank coupling** *(merged via flag)* | same git co-change data; `--group pairs\|clusters` selects aggregation **— see §3 caveat, this is the risky merge** |
| `rank budget` | **rank purposes** *(renamed)* | line-breakdown-by-purpose is a population measurement; rename kills the word collision |
| `analyze health / all / summary` | **rank summary** *(rollup)* | composite dashboards = the un-ranked digest of many rank metrics (awkward — see §5) |
| `trend complexity / length / density / test-ratio / multi` | **rank … --over-history** | a trend is a ranking with a time axis; folds into rank as a projection flag |
| `budget measure` | **check budget --measure** (or `rank purposes`-adjacent measure) | the measurement that feeds the verdict |
| `budget check / add / show / update / remove` | **check budget** (verdict) + **admin** (the CRUD) | the verdict is `check`; defining a budget is config admin |
| `ratchet check` | **check ratchet** | verdict against recorded baseline |
| `ratchet measure / add / show / update / remove` | **check ratchet --measure** + **admin** | same split as budget |
| `rules run` / `rules run --fix` | **check rules** | the verdict; `--fix` is a guarded mutation projection of it |
| `rules list / show / enable / disable / add / update / remove / setup / tags / validate / compile / test` | **admin** (rule config) | managing the ruleset is administration, not a query |
| `ci` | **check** (bare, = "run every check") | the all-checks verdict is the root of the `check` verb |
| `edit *` (delete/replace/swap/insert/rename/undo/redo/goto/batch/move/extract-function/…) | **edit** | unchanged |
| `edit history` | **edit log** *(renamed)* | "history" collides with `view history` (git); the undo log is a log |
| `daemon / serve / grammars / config / generate / sync / translate / docs(fetch) / package / sessions / kg / context / guide / init / update` | **admin** | irreducible plumbing (§5) |

---

## 3. The four specific issues — resolved

**(1) analyze/rank boundary — ELIMINATED, not redrawn.** There is no `analyze`.
The boundary caused bugs because two services could each hold a population metric,
so metrics migrated between them and guides rotted. SUBTRACT removes the second
home entirely: **if the result is a list of units with scores, it is `rank`, and
there is nowhere else it could go.** analyze's metric/finding commands move into
rank; analyze's dashboards become a rank rollup (`rank summary`); `architecture`
merges into `view graph`. A command cannot migrate between two services when only
one service can hold its shape. The rule is mechanical, not editorial.

**(2) `rank budget` vs `budget` collision — dissolved by shape.** They were never
the same kind of thing. `rank budget` returns a scored breakdown (a list) →
`rank`, renamed **`rank purposes`** to drop the loaded word. The `budget` service
returns a pass/fail verdict → `check budget`. The word "budget" now appears once,
as the noun of a `check`. Collision gone, and each command sits where its return
type says it must.

**(3) Near-duplicates.** SUBTRACT's instinct is *merge* — but only where the
return type is genuinely the same:
- `analyze architecture` vs `view graph`: identical computation (cycles, hubs).
  **Merge into `view graph`.** Both inspect the single dependency-graph artifact;
  there is one canonical home and `architecture` is retired.
- `analyze coupling-clusters` vs `rank coupling`: same git co-change data, two
  aggregations (file *pairs* vs connected-component *clusters*). **Merge into
  `rank coupling` with `--group pairs|clusters`.** **Caveat (load-bearing):** pairs
  and clusters are *different return types* (`Vec<Pair>` vs `Vec<Cluster>`).
  Cramming two types under one command via a flag is exactly the
  "wrap N report types in an enum" anti-pattern CLAUDE.md forbids. This merge is
  the place where the minimize frame starts to fight API-first — flagged honestly,
  not hidden. If the types won't reconcile, the correct SUBTRACT-compatible answer
  is to keep them as two `rank` subcommands (`rank coupling`, `rank coupling-clusters`)
  — still one verb, no second-service ambiguity — rather than force a lossy union.

**(4) `cfg cfg` — collapsed.** Control flow of one named function is single-artifact
inspection → **`view cfg <fn>`**. The empty service wrapper is deleted; cfg joins
the view family, consistent with "view inspects one thing you named."

---

## 4. Migration cost

**Commands that change path: ~45–50 of ~165 (~30%).**
- `analyze` (14) fully dissolves: ~9 → `rank`, 3 dashboards → `rank summary`,
  `architecture` → `view graph`, `coupling-clusters` → `rank coupling`.
- `rank` keeps its name but gains ~9 and renames 1 (`budget`→`purposes`).
- `budget` (6) + `ratchet` (6): verdict → `check`, CRUD → admin (~12 re-parents).
- `rules run` → `check rules`; `ci` → `check` (2).
- `cfg` (1), `syntax` (3), `trend` (5), `edit history`→`edit log` (1) move/rename.

**Blast radius:**
- Breaks muscle memory on the two highest-traffic services (`analyze`, `rank`)
  and on every CI invocation (`ci`, `budget check`, `ratchet check`, `rules run`).
- Breaks **every** guide, `LLMS.md`, `README.md`, and doc string referencing
  `analyze.*` (the guide system is *already* broken on the last analyze→rank move
  — H-4/H-5 — so this is re-paving a road that's already cratered).
- Breaks external user scripts and `.normalize` CI configs in the wild (only early
  users, pre-1.0).

**Flag-day, not incremental.** The analyze→rank merge *is* the design — you cannot
half-merge two services and still have one home per shape; during a partial
migration both homes exist and the bug returns. CLAUDE.md's "finish migrations
before building on top" makes the half-state the worst state. Recommend a single
breaking release with hidden one-release aliases *only* if real CI configs are
known to use the old paths (CLAUDE.md prefers retire-don't-deprecate, so default
to no aliases). The guide/doc rewrite must land in the same commit (the sync rule).

---

## 5. Honest trade-offs

**Where SUBTRACT is strong:**
- **Kills the bug class at the root.** The analyze/rank boundary produced *every*
  structural bug in the audit (migrations, stale guides, near-duplicates). One
  home per result-shape makes those migrations impossible by construction — far
  stronger than "write down a rule for which service gets the metric."
- **Four verbs map to four sentences** any user (human or agent) can run in their
  head: list? `rank`. verdict? `check`. one thing? `view`. change? `edit`.
- **Reinforces API-first** instead of fighting it: the verb *is* the return type,
  so the CLI grouping and the data model can never drift.

**Where SUBTRACT is thin:**
- **The admin tier doesn't reduce.** Roughly half the services (daemon, serve,
  grammars, config, generate, sessions, kg, context, package, guide, init, update,
  sync, structure-rebuild, the rules/budget/ratchet CRUD) are irreducibly
  heterogeneous plumbing. SUBTRACT has nothing to say about them except "not core."
  Forcing them under a fifth verb (`manage`/`admin`) would be a grab-bag — the
  exact arbitrariness we set out to remove. The frame shrinks the analytical core
  beautifully and then runs out of road.
- **Dashboards aren't lists.** `analyze health`/`summary` return a composite, not a
  `Vec<Scored<T>>`. Filing them under `rank` is a category fudge papered over with
  a `summary` subcommand. They arguably want their own shape — but a 5th core verb
  for "rollups" is a lot of taxonomy for three commands.
- **`rank` becomes huge** (~30 subcommands). We traded N shallow services for one
  deep one and now need *intra-rank* categories — which is the within-service
  grouping problem we started with, relocated one level down. Did we subtract
  concepts or just move the depth?

**The single biggest weakness for THIS problem:**
**Minimizing verbs pressures you to merge commands that return genuinely different
data shapes — which fights API-first, the one constraint the taxonomy must
respect.** The merge instinct that cleanly resolves `architecture`↔`view graph`
(same type) misfires on `coupling-clusters`↔`coupling` (pairs vs clusters — *two*
types) and on dashboards-vs-rankings. To hit "few verbs" you start unioning
distinct return types behind a flag, which is precisely the enum-wrapping
anti-pattern CLAUDE.md bans. SUBTRACT is excellent at *deleting* a false boundary
(analyze/rank) and dangerous when it tempts you to *manufacture* a false unity. The
discipline that saves it is the same one that motivates it — let the data shape
decide — which means the honest version of this candidate sometimes keeps two
subcommands under one verb rather than collapsing them, accepting a slightly larger
leaf count to avoid distorting the API.
