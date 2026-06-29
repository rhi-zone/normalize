# Candidate C — Organize by User Task / Workflow

**Frame:** optimize the CLI surface for *what the user is trying to DO* and for the
discoverability of the dominant workflows. Group commands by **intent**, accepting that
the underlying data shapes and implementations differ within a group.

This is a design proposal. Nothing here is implemented or committed.

---

## 0. Where the implicit task taxonomy already lives

normalize already ships a task taxonomy — it is just undocumented and unenforced:

- **Root `--help`** groups into Core / Analysis / Utilities / Infrastructure. These are
  loose *workflow* buckets, not data-shape buckets.
- **`guide <topic>`** is the cleanest existing statement of "what a user does":
  `explore` (understand a codebase), `analyze` (judge quality), `rules` (write/enforce
  checks), `setup` (configure the tool), `tree-sitter` (develop grammars/queries).

The guides are the strongest evidence for this frame: they already organize commands by
the *question the user is answering*, and they freely mix services to do it (the
`explore` guide pulls from `view`, `grep`, `analyze`, *and* `rank` in one workflow). The
problem the audit found — broken guides (`guide analyze` references 7 commands that
moved `analyze`→`rank`) — is precisely the failure mode of a task taxonomy that nobody
wrote down: each feature picked its own home by *perceived* task, the perception drifted,
and the guides rotted. **This candidate's whole job is to replace "perceived task" with a
mechanical membership rule so the grouping cannot drift again.**

---

## 1. The principle and the membership rule

### Principle (one sentence)

> A command is mounted under the **verb naming the question its own output answers**, where
> the verb is drawn from a closed set and chosen by a deterministic decision procedure over
> the command's *observable I/O properties* — never by a hand-wave about what task it "feels
> like."

The task label is the human-facing name (for discoverability); the decision procedure is
the machine-checkable membership rule (for stability).

### The closed verb set (top-level groups)

Eight task verbs plus two specialist domains. The set is **closed**: adding a new verb is a
documented taxonomy change, not a per-feature decision.

| Verb | The user's question | Membership signature |
|------|--------------------|----------------------|
| `view` | "What is *this specific thing* and what is it connected to?" | read-only · requires a pointed-at target (path/symbol) · output is the thing or its relationships, unordered |
| `grep` | "Where is the code that looks like X?" (kept top-level, iconic) | read-only · text search · target is a pattern, not a location |
| `rank` | "What is the **worst** code by metric M?" | read-only · primary payload is a `Vec` **sorted by a metric** (top-N / ordering is the point) |
| `analyze` | "How **healthy** is this scope, and what's structurally wrong?" | read-only · whole-scope judgment · payload is a structured report, **not** a metric-sorted list |
| `trend` | "Is metric M getting **better or worse over time**?" | read-only · primary axis is git history / commits |
| `edit` | "**Change** this code." | mutates **source files** |
| `check` | "Did anything **violate the bar**?" | produces a pass/fail **verdict** against a configured threshold; CRUD for those thresholds lives here too |
| `manage` | "Configure **normalize itself**." | mutates normalize's own state (index, config, grammars, daemon) **or** is read-only introspection of that state |
| `kg` | knowledge-graph CRUD | distinct persistent-graph data domain |
| `syntax` | grammar/query development | distinct tree-sitter-introspection domain |

### The membership decision procedure (objective, ordered)

Evaluate top to bottom; **first match wins** (the ordering is the tiebreak that makes it
deterministic, so a command that could plausibly fit two verbs always lands in the same one):

1. **Does it mutate source code on disk?** → `edit`
2. **Does it mutate normalize's own state (index / config / grammars / daemon / cache)?** → `manage`
3. **Is its primary result a pass/fail verdict against a configured threshold (or CRUD of
   those thresholds)?** → `check`
4. **Is its primary axis git history over time?** → `trend`
5. **Is its primary payload a `Vec` sorted by a metric (a ranking / top-N)?** → `rank`
6. **Does answering it require a pointed-at target (a path or symbol)?** → `view`
7. **Otherwise** (whole-scope, read-only, unordered judgment) → `analyze`

`grep`, `kg`, `syntax` are recognized first as named specialist domains (text-search
primitive; persistent-graph CRUD; grammar introspection) and skip the procedure.

The rule is objective because each test reads off a property you can observe without
opinion: *does it write files? does it return a sorted Vec? does it need a target argument?*
Two engineers running the procedure on the same command land in the same group. **That is
what makes this a rule-governed taxonomy and not another accretion.**

### Why this is stable where the current split is not

The analyze/rank boundary broke because "is complexity ranking an analysis or a ranking?"
is a *vibe*. Replace the vibe with rule 5 vs 7: **if the report's load-bearing payload is a
metric-sorted `Vec`, it is `rank`; if it is an unordered structured judgment, it is
`analyze`.** This is checkable against the report struct itself — a lint could enforce it
(`#[cli]` methods mounted under `rank` must return a type whose primary field is a sorted
collection). The taxonomy becomes a property of the code, not of memory.

---

## 2. Mapping table for the contested cross-section

Applying the procedure to the analyze/rank/cfg/budget/graph cross-section:

| Command today | Output property | Rule hit | Lands in | Change |
|---------------|-----------------|----------|----------|--------|
| `rank complexity / length / ceremony / uniqueness / call-complexity / duplicates / duplicate-types / fragments` | sorted Vec by metric | 5 | `rank` | unchanged |
| `rank size / density / imports / surface / depth-map / layering / module-health` | sorted Vec by metric | 5 | `rank` | unchanged |
| `rank files` | sorted Vec | 5 | `rank` | unchanged |
| `rank hotspots / coupling / ownership / contributors` | sorted Vec (uses git data, but axis is *ordering*, not *time*) | 5 (before 4? see note) | `rank` | unchanged |
| `rank test-ratio / test-gaps` | sorted Vec | 5 | `rank` | unchanged |
| `rank budget` | per-category line breakdown (composition, not a ranking *and* not a verdict) | — | `rank` | **rename → `rank composition`** (frees the word "budget" for the enforcement task) |
| `analyze health / summary / all` | whole-scope report, unordered | 7 | `analyze` | unchanged |
| `analyze architecture` | unordered report: cycles + hubs + coupling pairs | 7 | `analyze` | absorbs `view graph` (see below) |
| `analyze coupling-clusters` | unordered connected-component groups | 7 | `analyze` | unchanged |
| `analyze activity / repo-coupling / cross-repo-health` | whole-scope git report, not time-series | 7 | `analyze` | unchanged |
| `analyze security / liveness / effects / exceptions / docs / skeleton-diff` | whole-scope report | 7 | `analyze` | unchanged |
| `view graph` | graph properties (cycles, hubs, centrality) of the whole graph | overlaps `analyze architecture` | **merge into `analyze architecture`** | retire `view graph`; fold centrality into the report |
| `cfg cfg` | control-flow graph of *one named function* | 6 (needs a target) | `view` | **collapse → `view cfg <fn>`** |
| `budget` (measure/add/check/show/remove) | pass/fail verdict + CRUD of diff-size thresholds | 3 | `check` | **move → `check budget`** |
| `ratchet` (measure/add/check/...) | pass/fail verdict + CRUD of regression baselines | 3 | `check` | **move → `check ratchet`** |
| `ci` | runs all checks, pass/fail | 3 | `check` | **move → `check ci`** (or keep `ci` as a documented top-level alias for CI ergonomics) |
| `rules run` | pass/fail verdict | 3 | `check` | rules engine moves under `check`; see §3 note |
| `trend *` | metric over commits | 4 | `trend` | unchanged |

**Note on rule ordering (4 vs 5) for git-history commands.** `rank hotspots` *uses* git
churn but its output is a *ranking* — the user's question is "which files are worst," not
"how did churn change." `trend complexity` *is* a time-series — "is it getting worse." The
procedure separates them cleanly because rule 4 ("primary axis is history-over-time") only
fires when the result is indexed by commit/time, which `rank hotspots` is not. This is the
exact ambiguity that sank the old split; the procedure resolves it by reading the output
shape, not the input data source.

---

## 3. Resolving the four specific issues

**(1) analyze/rank boundary undefined.** Resolved by rules 5 vs 7: *metric-sorted `Vec` →
`rank`; unordered scope judgment → `analyze`.* This is mechanical and lint-enforceable, and
it is exactly the rule whose absence caused the migration that broke `guide analyze`.
Stragglers that crossed the line get pulled back by the procedure (none currently need to
move — the existing membership already satisfies the rule; the rule just *names* it so the
next feature can't drift). The guides regenerate against the real tree and a `guide test`
snapshot prevents re-rot (already recommended in the audit as T1-6).

**(2) `rank budget` vs `budget` collision.** The word "budget" denotes *one* task —
enforcing a bar (rule 3) — so it belongs exclusively to the `check` family: `check budget`.
The `rank budget` command is not a budget at all; it is a line-composition breakdown, so it
renames to **`rank composition`** (or `rank purposes`). The collision dissolves because the
two commands were never the same task; the task frame forces the word to mean one thing.

**(3) `analyze architecture` vs `view graph` near-duplicate.** Both answer the *same user
question* — "what is the shape of my dependency graph and what's wrong with it?" One task =
one command. **Merge:** retire `view graph`, fold its centrality metrics into the
`analyze architecture` report. (Rule 6 would otherwise want a graph-of-a-target under
`view`, but `view graph` has no single target — it is whole-graph — so rule 7 sends it to
`analyze`, where `architecture` already lives. The procedure and the merge agree.)

**(4) `cfg cfg` double-wrap.** A control-flow graph is *of one named function* — a pointed-at
target — so rule 6 puts it under `view`: **`view cfg <fn>`**. The redundant service wrapper
disappears because in the task frame, "show me how this function executes" is the same task
as everything else under `view` (read/understand a specific thing). No standalone `cfg`
service is justified.

---

## 4. Migration cost, blast radius, flag-day vs incremental

### Renames / moves (the full set)

```
cfg cfg <fn>            → view cfg <fn>            (collapse double-wrap)
rank budget             → rank composition         (free the word "budget")
budget *                → check budget *           (enforcement cluster)
ratchet *               → check ratchet *
ci                      → check ci                 (keep `ci` as documented alias)
rules *                 → check rules *            (engine + run; see caveat)
view graph              → (retire; merge into analyze architecture)
edit history *          → edit log *               (audit T2-10, free of this frame but adjacent)
```

`view`, `grep`, `trend`, `edit`, `analyze`, `rank` (post-rename), `syntax`, `kg` keep their
top-level names. `manage` is a **new umbrella** over the current Infrastructure +
config/index/setup leaves (`init`, `structure`, `grammars`, `config`, `daemon`, `serve`,
`update`, `generate`, `package`, `context`, `sync`, `aliases`). This is the largest
re-mount but the *least* contentious — those commands already cluster as "configure the
tool" and nobody navigates to them by guessing.

**Caveat on `rules`.** Moving `rules` under `check` is the one move with real downside:
`rules` is a large, well-regarded service (the audit calls it "well-designed"), it owns
`enable/disable/show/add/...` which are *configuration*, not verdicts, and the pre-commit
hook + every guide references `rules run`. Recommendation: **keep `rules` top-level** and
treat `check` as the verdict-producing *entry points* (`check budget`, `check ratchet`,
`check ci`, and `ci` calls `rules run` internally). This is an honest exception:
`rules` straddles `check` (run → verdict) and `manage` (enable/disable → config), and
splitting a coherent, popular service to satisfy the procedure costs more than it returns.
Document the exception rather than force the move (per CLAUDE.md: a procedure that would
shred a coherent service is the procedure misapplied).

### Blast radius

- **Docs that must change in the same release:** `docs/cli/`, `README.md`, `LLMS.md`,
  `docs/cli-design.md`, all five `guide` bodies.
- **Tests:** `generate cli-snapshot` regenerates once; `assert_output_formatter` tests
  follow report structs (report types don't move — see §5 — so these are low-touch).
- **Installer / external:** install.sh unaffected (no command names baked in). Any user CI
  invoking `budget`/`ratchet`/`ci` directly breaks — but pre-1.0, retire-don't-deprecate.
- **Service-layer code:** *minimal* — moves are changes to `#[cli(...)]` mount paths and
  command dispatch, not to the service crates (see §5). `cfg cfg`→`view cfg` and the
  `view graph`/`architecture` merge are the only ones touching report logic.

### Flag-day vs incremental

**Flag-day, single release.** CLAUDE.md mandates retire-don't-deprecate and forbids
backward-compat aliases; incremental renames would (a) leave the `rank budget`/`budget`
collision live longer and (b) require dual-mounting, which is exactly the "N aliases"
anti-pattern the project bans. One release: rename + move + regenerate cli-snapshot + rewrite
guides + one CHANGELOG block under `[Unreleased]`. The `ci` alias is the *only* sanctioned
retained name, justified by CI-pipeline ergonomics.

---

## 5. How a task projection coexists with API-first / library-first

This is the central tension and it has a clean answer: **the verb is a mount point, not a
type.**

- The service layer stays **shape-oriented**: one report struct per data shape, owned by the
  crate that computes it (`normalize-budget` owns the budget report; `normalize-rank` owns
  sorted-Vec reports). The typed library is the source of truth, unchanged.
- The CLI is a **projection** that mounts each `#[cli(...)]` service method under a
  task-named path. `check budget` and `check ratchet` can pull methods from *different
  crates*; `view cfg` mounts a method that lives wherever control-flow analysis lives. The
  task path differs from crate ownership — which is *already* true (commands have moved
  between services before; that capability is what we're now governing).
- **Membership keys off the report shape, so the projection never distorts the data model.**
  Rule 5 ("sorted Vec → rank") is a *consequence* of the report type, not a constraint on it.
  We don't force unlike data under one report struct; we group unlike report structs under one
  *verb* when they answer one *question*. That is the legal move (group the surface); the
  illegal move CLAUDE.md forbids (wrap N report types in an enum) we do **not** make — `check`
  is a mount namespace, not a union type.

So: a task-oriented projection *can* sit over shape-oriented services precisely because the
CLI surface and the type system are different layers. The decision procedure is the bridge —
it derives the (task) mount path mechanically from (shape) properties of the typed result.

### Index-first reality

The procedure deliberately **does not** cut on single-file vs index-requiring. Within `view`,
`chunk`/`history`/`blame`/`cfg` are single-file while `referenced-by`/`dependents`/
`import-path` require the index; within `rank`, `complexity` is single-file while `imports`/
`layering`/`depth-map` require it. **This is correct for a task frame:** the user's task is
"understand this code's relationships," and whether the answer needs an index is an
*implementation prerequisite*, not a user-facing concern. The index boundary is handled at
the **error layer** (audit T1-1: missing-index commands must exit non-zero with
`requires_index: true` and "run `structure rebuild`"), not at the taxonomy layer. Surfacing
the index split in the command *tree* would force the user to know implementation details to
find the command — the opposite of the task frame's goal. Condition: the missing-index error
must be excellent, because the taxonomy is intentionally hiding the boundary.

---

## 6. Honest trade-offs

### Where task organization is strong

- **Matches how users (human and agent) actually think.** Nobody asks "which command returns
  a sorted Vec"; they ask "what's my worst code" (`rank`) or "is this healthy" (`analyze`).
  The guides already prove the dominant workflows are task-shaped.
- **Discoverability of dominant workflows.** The five guides map 1:1 onto verb clusters, so
  "I want to do X" → one verb → its subcommands. An agent reading `view --help` finds every
  way to look at a specific thing in one place, index-required or not.
- **The collision and double-wrap resolve *for a reason*, not by fiat.** "budget" means one
  task; "cfg of a function" is an inspect task; "graph shape" is one question. The frame
  gives a *principled* answer to each of the four issues, and the answers agree with the
  mechanical procedure.

### Where it is thin (the real weaknesses)

- **Subjectivity and drift — the biggest weakness for THIS problem.** Task taxonomies drift
  because "what task is this?" is a vibe, and *that vibe-driven drift is exactly how the
  current mess accreted* (the analyze→rank migration that rotted the guides). My mitigation
  is the observable-I/O decision procedure, but it is a mitigation, not a cure: the procedure
  is only as objective as its tests, and "is the primary payload a sorted Vec" has edge cases
  (a report that is *part* ranking, *part* summary — e.g. `module-health`). The honest claim
  is *"more rule-governed than the status quo and lint-enforceable,"* not *"drift-proof."* A
  shape-first taxonomy (Candidate B, presumably) would be *more* objective here, at the cost
  of matching how users think — that is the core trade this candidate makes.
- **Commands that serve multiple tasks.** `view references` is an inspect primitive *and* the
  input to dead-code assessment; `translate` is read-to-understand *and* produce-code; `rules`
  straddles `check` and `manage`. The frame forces a single mount (no multi-mounting, by
  design — multi-mounting is how aliases breed), which means some commands live somewhere a
  user might not look first. The `rules`-under-`check` move is the sharpest case and I
  recommend *not* making it — a documented exception, which is itself an admission that the
  procedure has soft edges.
- **The API-first tension is managed, not eliminated.** Keying membership off report shape
  keeps the projection faithful, but it also means a *future* change to a report's shape
  could silently change its correct mount point. That coupling is the price of making the
  rule mechanical. A lint that flags "method mounted under `rank` whose return type's primary
  field is not a sorted collection" turns the tension into a CI failure rather than a slow
  drift — recommended if this candidate wins.
- **`manage` is a grab-bag.** Eight-plus unlike commands under one "configure the tool" verb
  is coherent as a *task* ("set up / operate normalize") but weak as a *namespace* — it is
  the bucket where the procedure's rule 2 sweeps everything that isn't about the user's code.
  Defensible (nobody discovers `daemon` by guessing) but it is the least elegant part.

### Bottom line for this frame

The task frame's strength is that it *is* how the guides — the project's own
statement of intent — already organize the surface, and it gives principled, agreeing
answers to all four issues. Its weakness is endemic and on-point: **task taxonomies drift,
and drift is the documented cause of the exact mess we are fixing.** This candidate is only
viable *with* the mechanical decision procedure and a lint enforcing it; sold as "group by
intent" alone, it would re-accrete within a few releases. The procedure converts the
subjective frame into a checkable one — that is the whole bet, and the residual soft edges
(`module-health`, `rules`, `manage`) are where the bet is weakest.
