# Candidate D — Organize by Input Scope / Prerequisite

*CLI taxonomy design, 2026-06-29. Design only — not implemented, not committed.*
*Frame: the top-level verb encodes **what the command consumes** — and therefore
**what you must have built before it can answer**.*

---

## 1. The principle

> **A command's top-level verb is the strictest prerequisite it requires.**
> Reading the verb tells you the input scope — a single file, the built index,
> git history, multiple repos, or project state — and therefore whether you need
> to run `structure rebuild` (or be in a git repo, or have configured peers)
> before the command can produce a meaningful answer.

This is the project's *already-half-lived* index-first boundary made explicit and
load-bearing. CLAUDE.md already states it ("Single-file commands … work without the
index; cross-file features … require it and prompt the user to run `normalize
structure rebuild`"). Today that boundary is invisible in the command tree:
`view graph` needs the import graph, `view <file>` does not, and nothing in the
verb tells you which. Candidate D promotes the prerequisite from a runtime surprise
(or, pre-T1-1, a silent empty result) to a syntactic property of the verb.

### The scope → verb taxonomy (read-input axis)

The scopes are not invented; they are the distinct prerequisite tiers that already
exist in `crates/normalize/src/index.rs` plus the git/cross-repo/state inputs the
services actually consume. Five read-input scopes, five verbs:

| # | Scope | Prerequisite (grounded) | Verb | Replaces |
|---|-------|------------------------|------|----------|
| S0 | **Single file / symbol** | none — the file *is* the input | `view` | `view` (file ops), `cfg`, single-file `syntax`, single-file `rank` metrics |
| S1+S2 | **The built index** (symbols/calls and/or import graph) | `structure rebuild` (`ensure_ready` / `require_import_graph`) | `index` | `rank` + the index-needing half of `analyze` |
| S3 | **Git history** | a git repo with history (`git_utils`, blame, revwalk) | `history` | `view history`/`blame`, the git half of `analyze`, all of `trend`, git-`rank` |
| S4 | **Multiple repos** | configured peer repos (`multi_repo.rs`) | `fleet` | `analyze repo-coupling`, `analyze cross-repo-health` |
| S5 | **Project state / config** | a writable `.normalize/` (or reads it) | `config` | `config`, `budget`, `ratchet`, rules *management*, `aliases`, `kg` |

Plus two axes that are **not** input-scope and are handled separately (see §5):
`serve`/`daemon`/`grammars`/`update` (environment & process — no read-input), and
**mutation** (`edit`, `structure rebuild`, `rules run --fix`) which is an *output*
axis orthogonal to input scope.

### The membership rule

**Strictest-prerequisite-wins, and it is monotonic.** A command lives under the
verb for the highest prerequisite tier it can require on any code path:

```
needs ≥2 repos      → fleet
else needs git log  → history
else needs index    → index      (S1 symbols OR S2 import-graph — same verb; see §5)
else needs only a file → view
else only touches .normalize → config
```

Two consequences that make this objective rather than aesthetic:

1. **It is decidable by inspection of the code, not taste.** `require_import_graph`
   in the body ⇒ `index`. `git_utils` ⇒ `history`. `multi_repo` ⇒ `fleet`. The
   compiler-visible dependency *is* the classification. This is why the
   `analyze`/`rank` migration bugs (H-4/H-5: commands moved, guides not updated)
   cannot recur — there is no judgment call about which of two equivalent verbs a
   command "belongs" to.
2. **The verb answers "must I rebuild first?" before I run anything.** Every
   `index *` command needs `structure rebuild`; no `view *` command does. That is
   the whole pitch: the prerequisite stops being a runtime trap (T1-1) and becomes a
   spelling.

---

## 2. Mapping table for the contested cross-section

Classified by *actual* input scope, grounded in the index-tier each service method
calls (`crates/normalize/src/index.rs`, `service/{view,rank,analyze}.rs`,
`commands/analyze/*`).

| Today | Index tier in source | Scope | New home |
|-------|---------------------|-------|----------|
| `view <file>` / `chunk` / `list` | none | S0 | `view <file>` / `view chunk` / `view list` |
| `view trace` | `open_if_enabled` (enriches) | S0* | `view trace` (see mixed-scope, §5) |
| `view history` / `view blame` | git | S3 | `history file` / `history blame` |
| `view graph` | `require_import_graph` | S2 | `index graph` |
| `view dependents` / `import-path` | `require_import_graph` | S2 | `index dependents` / `index import-path` |
| `view referenced-by` / `references` | `ensure_ready` | S1 | `index callers` / `index callees` |
| `cfg cfg` | none (single file CFG) | S0 | `view cfg` |
| `rank complexity` / `length` / `ceremony` / `uniqueness` / `fragments` | none (per-file metric) | S0 | `view` metrics — see §5 note | 
| `rank imports` / `depth-map` / `layering` | `require_import_graph` | S2 | `index imports` / `depth-map` / `layering` |
| `rank surface` / `density` / `size` / `module-health` | `ensure_ready` | S1 | `index surface` / … |
| `rank call-complexity` | `ensure_ready` | S1 | `index call-complexity` |
| `rank duplicates` / `duplicate-types` | index (corpus) | S1 | `index duplicates` |
| `rank hotspots` / `coupling` / `ownership` / `contributors` | git | S3 | `history hotspots` / `coupling` / `ownership` / `contributors` |
| `rank test-ratio` / `test-gaps` | `ensure_ready` | S1 | `index test-ratio` / `test-gaps` |
| `rank budget` (line breakdown) | source/index | S1 | `index purposes` |
| `analyze architecture` | `require_import_graph` | S2 | `index graph` (merged with `view graph` — see §3) |
| `analyze summary` / `health` / `all` / `liveness` / `effects` / `exceptions` | `ensure_ready` | S1 | `index summary` / `health` / … |
| `analyze docs` | `ensure_ready_or_warn` | S1 | `index docs` |
| `analyze coupling-clusters` | git (`ensure_ready_or_warn` enrich) | S3 | `history coupling --cluster` (merged — §3) |
| `analyze activity` | git | S3 | `history activity` |
| `analyze skeleton-diff` | git (ref) | S3 | `history skeleton-diff` |
| `analyze repo-coupling` | multi-repo + git | S4 | `fleet coupling` |
| `analyze cross-repo-health` | multi-repo | S4 | `fleet health` |
| `analyze security` | index/source | S1 | `index security` |
| `trend *` | git (worktree checkout) | S3 | `history trend <metric>` |
| `budget` (diff-size CRUD) | `.normalize` config | S5 | `config budget` |
| `ratchet` | `.normalize` config | S5 | `config ratchet` |
| `config` | `.normalize` config | S5 | `config` (already correct) |
| `kg read/write/walk` | `.normalize` state | S5 | `config kg` (or its own; state-scoped) |

`*` Mixed scope — see §5.

The striking structural fact: **`rank` and `analyze` collapse into the same verb**
because they share the same prerequisite (the index). Every documented confusion
between them is a confusion about *what they compute*, never about *what they
consume* — and the frame organizes by the latter.

---

## 3. Resolving the four specific issues

**(a) analyze/rank boundary undefined (H-4/H-5 broke guides).**
*Dissolved, not redrawn.* `analyze` and `rank` both consume the index, so under
the scope rule they are **one verb, `index`**. The quality-vs-ranking distinction
that the two services were straining to encode becomes a *help-text grouping inside
`index`* (e.g. `index --help` lists "Quality", "Structure", "Coverage"
sub-sections), not a verb boundary. Moving a command between those groups changes
its help-text section, **not its invocation path** — so the class of bug behind
H-4/H-5 (a command's path silently changing, guides left stale) is structurally
impossible. There is nothing to migrate *across*. This is candidate D's single
biggest win.

**(b) `rank budget` vs `budget` collision (T2-6).**
Different input scopes, so different verbs, so no collision — *even if the words
stayed the same*. `rank budget` consumes source/index (line-count breakdown by
purpose) ⇒ `index purposes`. `budget` is CRUD over `.normalize` diff-size limits ⇒
`config budget`. `index purposes` and `config budget` cannot be confused: the verb
already tells you one reads code and one reads/writes config. The rename to
`purposes` is then a clarity bonus, not the load-bearing fix.

**(c) Near-duplicates.**
- `analyze architecture` vs `view graph`: both require the **import graph** (S2),
  so both land at `index graph`. The frame *forces them adjacent under one verb*,
  which is exactly the condition under which a duplicate is impossible to miss —
  you cannot have two sibling subcommands named `graph`. They merge into one
  `index graph` (centrality + coupling-pairs + cycles as flags/sections of one
  report). The shared `require_import_graph` prerequisite is what proves they are
  the same feature.
- `analyze coupling-clusters` vs `rank coupling`: both consume **git co-change
  history** (S3) ⇒ both land under `history`. Same forced adjacency ⇒ merge into
  `history coupling`, with cluster aggregation as `--cluster` (or a `--by
  pair|cluster` flag). One git pass, two aggregations, one command.

**(d) `cfg cfg` double-wrap (T2-7).**
`cfg` builds the control-flow graph of a **single file** — pure S0, no index, no
git. Its scope is identical to `view`'s. So it becomes `view cfg <file>`,
collapsing the redundant service wrapper and landing it in the verb that already
means "I consume one file." The double-wrap existed only because `cfg` was given
its own top-level service; scope organization gives it an obvious parent.

---

## 4. Migration cost, blast radius, sequencing

**Blast radius: very large — this is the most invasive of the candidate frames.**
Nearly every top-level verb changes:
- `rank *` (≈22 leaves) → `index *` / `history *`
- `analyze *` (≈14 leaves) → `index *` / `history *` / `fleet *`
- `trend *` (5) → `history trend *`
- `view history`/`blame` → `history *`
- `view graph`/`dependents`/`import-path` → `index *`
- `budget`/`ratchet` → `config *`
- `cfg cfg` → `view cfg`

Counting leaves whose *invocation string* changes: ~55–60 of ~165 (~35%). This is a
genuine flag-day-scale rename, larger than candidates that keep `analyze`/`rank` and
only fix the four bugs.

**But the migration is one-time and *anti-fragile against future churn.*** The
reason `analyze`/`rank` kept breaking guides is that the boundary was a judgment
call, so commands kept being re-sorted. Scope boundaries are decidable from the
code (the `require_import_graph` / `git_utils` / `multi_repo` dependency), so after
this migration there is no recurring re-sort: a command's verb is a function of its
prerequisites, which only change when its implementation changes.

**Incremental vs flag-day:** **flag day**, consistent with "retire, don't
deprecate" (pre-1.0). An incremental path (alias old verbs to new) would reintroduce
exactly the two-paths-for-one-command ambiguity the frame is trying to kill, and
would leave the prerequisite invisible on the legacy spelling. Do it in one release:
rename, update `docs/cli/`, `README.md`, `LLMS.md`, `docs/cli-design.md`, the
`guide *` bodies, and add the `guide test`/snapshot that T1-6 calls for (which now
*can* assert "every `index *` command errors cleanly without an index" because that
property is uniform across the verb).

**Cost not to hide:** the `index` help page becomes long (~25 subcommands). That is
the grab-bag, addressed next.

---

## 5. Honest trade-offs

### Where the frame is strong
- **The index prerequisite becomes self-documenting.** This is the headline and it
  is real: `index *` ⇒ "run `structure rebuild` first"; `view *` ⇒ "works
  anywhere"; `history *` ⇒ "needs git"; `fleet *` ⇒ "needs peers." The T1-1 class of
  bug (silent empty on missing prerequisite) maps to one uniform guard *per verb*,
  not per command — `index` can enforce `require_import_graph`/`ensure_ready` at the
  verb dispatch layer.
- **Objective membership rule.** Classification is a function of the code's actual
  dependencies, not of taste. This is what permanently kills the H-4/H-5 migration-
  churn bug.
- **The four issues resolve as *consequences* of the principle**, not as four
  bespoke patches. The dups are forced adjacent (so they merge); the collision is
  separated by scope (so it can't collide); the boundary dissolves (so there's
  nothing to migrate across).

### Where the frame is thin (and the central weakness)
- **The index grab-bag — this is the frame's biggest weakness for THIS problem.**
  A literal scope taxonomy gives *one verb per scope*, and the index scope contains
  ~25 commands that compute wildly different things (complexity, coupling, dead
  code, test gaps, layering, duplicates, security). `index` becomes the new
  `analyze` — a 25-entry junk drawer — just renamed. The frame **demotes** the
  quality/structure/coverage distinction from a verb boundary to a help-text
  grouping; it does **not eliminate** it. So the hardest organizational decision
  (how to sub-divide "everything that needs the index") is still there — and worse,
  the frame makes it *cheap to get wrong*, because a mis-grouping is now just a
  cosmetic section label nobody tests, rather than a path that a guide would catch.
  Candidate D's honest claim is narrow: it removes the *invocation-breaking* version
  of this decision, not the decision itself.
- **Splitting the index group by prerequisite doesn't help.** One might try to split
  `index` into "needs symbols" (S1) vs "needs import graph" (S2) to shrink the
  grab-bag along a prerequisite line. But both tiers are built by the *same*
  `structure rebuild`, so from the user's standpoint the prerequisite message is
  identical — the split would buy a smaller help page at the cost of a distinction
  with no user-visible prerequisite meaning, violating the frame's own rationale.
  So the frame is stuck with the grab-bag *or* with a non-scope sub-axis.
- **Mixed-scope commands force a lie.** `view trace` / `view search` / `analyze
  docs` work on a single file but *enrich* from the index if present
  (`open_if_enabled` / `ensure_ready_or_warn`). The monotonic strictest-wins rule
  files them under `index`, advertising a prerequisite they don't strictly need —
  the inverse of T1-1 (now we *over*-state the requirement). The alternative (file
  them under `view`) breaks the "verb ⇒ prerequisite" guarantee the whole frame
  rests on. There is no clean answer; this is intrinsic to organizing by a
  prerequisite that some commands hold *optionally*.
- **Per-file metrics blur S0/S1.** `rank complexity`/`length`/`ceremony` are
  per-file and need no index — pure S0 — yet they read as "ranking the codebase,"
  which *feels* like index scope. Putting `complexity` under `view` (S0, correct by
  the rule) while `surface` is under `index` (S1) splits an intuitively-single
  "code quality metrics" family across two verbs purely on the implementation detail
  of whether the metric is computed per-file or needs cross-file resolution. Scope-
  correct, discoverability-poor.
- **Mutation is orthogonal and unmodeled.** The frame indexes on *read* input.
  `edit *`, `structure rebuild`, `rules run --fix` are defined by what they *write*,
  not what they read (and several `edit` refactors need the index too). They sit
  outside the five read-scopes and need a parallel treatment, so the taxonomy is not
  actually single-axis — it is "scope for readers, something-else for writers."

### Net
Candidate D's principle is the most *objective* of the frames and it genuinely
dissolves the analyze/rank boundary bug at the root by making the prerequisite the
verb. Its fatal-if-unaddressed weakness is that the index scope is too populous to be
a single verb, and scope offers no honest sub-axis to break it up — so the frame
either ships a 25-command `index` grab-bag or smuggles the very what-it-computes
distinction it claimed to eliminate back in as untested help-text sections. The frame
is strongest if paired with a *secondary* organizing axis inside `index` (borrowed
from another candidate), and weakest if asked to be the sole organizing principle.
