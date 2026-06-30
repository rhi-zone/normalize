> **SUPERSEDED (2026-06-30) by `00-inversion-plan.md`.** The axis decision changed from
> output-shape to crate-ownership (top-level verb = owning compute crate). This shape plan
> is retained for its candidate/judge synthesis and contested-command analysis, but the
> implementation path is the inversion plan.

# 00 — Authoritative FULL-RETREE Plan: normalize CLI Command Taxonomy

*2026-06-29. Decision record + concrete command→new-home mapping. Implementation
spec, not implementation. Branch: `feat/cli-globals-pretty-wiring`.*

**Status: the decision is made — we ARE doing a full retree.** This document is the
synthesis the four candidates (A–D) and three judges converged on, turned into a
concrete, implementable mapping. It does not relitigate the axis choice.

## The decision being implemented (from the judges, not re-argued here)

1. **Primary membership axis = output SHAPE** — the only objectively, lint-enforceable
   rule. `RankEntry` (`crates/normalize-analyze/src/ranked.rs:97`) is machine-detectable;
   "a command mounted under `rank` must return a report whose load-bearing field is
   `Vec<T: RankEntry>`" is real and already flags `rank size` as misplaced (its
   `SizeNode` does not impl `RankEntry` — verified `commands/analyze/size.rs`).
2. **Verb NAMES are human-guessable; structure is two-level (verb + topic).** The shape
   axis is the *rule*, never the *name the user types*. We do **not** introduce `graph`
   or `tree` as top-level verbs (usability judge: they are the only B verbs nobody
   guesses). Graph-shaped and tree-shaped reports get human homes under `view`.
3. **`analyze` is DISSOLVED as a verb** — it is a topic masquerading as a verb and the
   re-accretion vector. Each member routes to its true-shape home.
4. **NO enum-wrapping** (CLAUDE.md ban). Unlike shapes stay separate commands. The only
   sanctioned merge is `analyze architecture` → `view graph` as a genuine single-report
   struct-union (same whole-import-graph domain).
5. **Transitional one-release aliases are allowed** (migration scaffolding, not permanent
   cruft) — removed at 1.0.
6. **D's input-scope axis is rejected** (not lint-enforceable — prereqs are buried/
   silently-degrading; `index` grab-bag).

---

## 1. Final verb set

Six analysis verbs (shape-grounded, human-named) + the kept specialist/admin domains.

### 1.1 Analysis verbs

| Verb | The user's question | Objective membership rule (shape) | Lint-checkability |
|------|--------------------|-----------------------------------|-------------------|
| **`rank`** | "What's the *worst* code by metric M?" | Load-bearing field is `Vec<T>` with `T: RankEntry`, sorted by a score. | **STRONG.** `RankEntry` is a real trait; enforce via a `RankedReport` bound (§4). Catches `rank size` today. |
| **`view`** | "Show me *this one thing* (file/symbol/derived structure) and what it's connected to." | A structured representation of **one** artifact: a named target (file/symbol/AST), or **one** derived whole-codebase structure (the import graph, a tree, a cluster graph, a CFG, a structural diff). Must NOT be `Vec<T: RankEntry>` and must NOT be a verdict. | **MEDIUM (exclusionary).** Lint asserts what `view` is *not* (not a `RankedReport`, not a `Verdict`) — catches drift back toward rank/check. |
| **`check`** | "Did anything violate the bar? Is something wrong?" | Returns a verdict/diagnostic: pass/fail + findings + non-zero exit on failure. | **MEDIUM.** Enforce via a `Verdict`/`Diagnostic` trait (pass/fail + exit code). |
| **`trend`** | "Is metric M getting better or worse over time?" | Primary axis is git history / commits: `Vec<Point>` indexed by time + delta/direction. | **MEDIUM.** Enforce via a `TimeSeriesReport` trait (points + delta). |
| **`overview`** *(name OPEN — §8)* | "Give me the whole-codebase digest." | A composite/aggregate dashboard: mixed scalars + embedded sub-reports, **no** single `Vec<T: RankEntry>` primary field, **no** verdict. The small bounded special-case (3 commands). | **WEAK (exclusionary).** A bounded allow-list of ≤3 members + lint asserting "not a `RankedReport`, not a `Verdict`." |
| **`edit`** | "Change this code." | Mutates source files; returns a `PlannedEdit`/applied diff with `--dry-run`. | **STRONG.** Mutation is a distinct surface (already `fix` feature-gated). |

The objective seam that matters most — the one that broke (analyze↔rank, H-4/H-5) —
is `rank` vs everything else, and it is the *strongest* lint. `overview` is the honest
residual but it is **bounded and enumerated** (3 commands), not an open "else" bucket;
that is the difference between it and the `analyze` it replaces.

### 1.2 Specialist / domain / admin verbs (KEPT as-is)

These were never the bug locus (resource-CRUD / infra / primitives). They keep their
own `#[cli]` services in their owning crates, per CLAUDE.md ("a crate that owns a
subcommand includes its own `#[cli]` service").

`grep`, `config`, `rules` (minus `run` — §3), `budget` (CRUD+`measure`, minus `check`),
`ratchet` (CRUD+`measure`, minus `check`), `structure`, `syntax`, `kg`, `daemon`,
`serve`, `grammars`, `generate`, `package`, `sessions`, `context`, `tools`, `guide`,
`init`, `update`, `aliases`, `translate`, `docs` (the doc-fetch utility), `sync`.

---

## 2. Two-level topic structure

The shipped tree is **already** two-level (`rank --help` and `analyze --help` ship topic
categories today via `#[server(groups(...))]`). The retree keeps the second level and
makes it **load-bearing and snapshot-tested** so it can't rot like the guides did.

Mechanism (existing): `#[server(groups(code = "Code quality", …))]` on the impl block
declares the topic set; `#[server(group = "code")]` on each method assigns it. Enforced
by a snapshot test of `--help` / `--manual` output (§6 ties into the same harness).

| Verb | Topic groups (second level) |
|------|------------------------------|
| `rank` (~22) | **Code quality** (complexity, ceremony, length, uniqueness, call-complexity, duplicates, duplicate-types, fragments) · **Module structure** (density, imports, surface, depth-map, layering, module-health, purposes) · **Repository** (files) · **Git history** (hotspots, coupling, ownership, contributors) · **Testing** (test-ratio, test-gaps) · **Cross-repo** (cross-repo-health) |
| `view` (~18) | **Source** (default-target, chunk, list) · **Call graph** (references, referenced-by, dependents, import-path) · **Graph & structure** (graph, cfg, size, coupling-clusters, repo-coupling) · **Git** (history, blame) · **Dataflow** (trace, liveness, effects, exceptions) · **Diff** (skeleton-diff) |
| `check` (~7) | **Gates** (budget, ratchet, rules, bare=all) · **Scans** (security, docs) |
| `trend` (~6) | **Metrics** (complexity, length, density, test-ratio, multi) · **Repository** (activity) |
| `overview` (3) | (flat) health, summary, all |

**Topic axis = topic-within-verb (matches what ships, matches the guides), NOT
scope-within-verb** (usability judge: scope is an implementation detail; reject).

---

## 3. Complete mapping table

Every current command → (new verb, topic, new name) or "unchanged". Grounded in the
actual return struct where contested (verified 2026-06-29).

### 3.1 `rank` — keep verb, two changes out, one rename, one gain

| Today | New home | Why (shape) |
|-------|----------|-------------|
| `rank complexity / ceremony / length / uniqueness / call-complexity / duplicates / duplicate-types / fragments` | **unchanged** | `Vec<T: RankEntry>` |
| `rank density / imports / surface / depth-map / layering / module-health` | **unchanged** | `Vec<T: RankEntry>` |
| `rank files / hotspots / coupling / ownership / contributors / test-ratio / test-gaps` | **unchanged** | `Vec<T: RankEntry>` |
| `rank size` | **`view size`** | `SizeReport.tree: Vec<SizeNode>`, `SizeNode` is a tree and does **not** impl `RankEntry` — the lint flags it. Tree = single derived structure → `view`. |
| `rank budget` | **`rank purposes`** (rename) | `LineBudgetReport` has `ModuleBudget: RankEntry` ranked tables → stays `rank`; rename resolves the `budget` word-collision (H-2). |
| *(gains)* `rank cross-repo-health` | from `analyze` | `CrossRepoHealthReport { repos: Vec<RepoHealthEntry> }` sorted by score → ranked list. **Impl note:** add `impl RankEntry for RepoHealthEntry` to satisfy the lint. |

### 3.2 `analyze` — DISSOLVED (all 14 members re-homed)

| Today | New home | Why (shape, verified) |
|-------|----------|----------------------|
| `analyze health` | **`overview health`** | composite dashboard, no `Vec<RankEntry>` primary. |
| `analyze summary` | **`overview summary`** | composite dashboard ("auto-generated overview"). |
| `analyze all` | **`overview all`** | composite (runs all passes). |
| `analyze security` | **`check security`** | `SecurityReport` = findings (secrets/unsafe patterns) → diagnostic verdict. |
| `analyze docs` | **`check docs`** | `DocCoverageReport` = coverage % + by-language + `worst_files` — dashboard-led coverage diagnostic, **not** a pure `Vec<RankEntry>` (primary field is a scalar coverage %). Routes to `check` (coverage gate). *Watch:* it embeds a ranked Vec; if it is later refactored so the ranked list becomes primary, the lint will push it to `rank` — acceptable. |
| `analyze liveness` | **`view liveness`** | `LivenessReport` = per-basic-block dataflow of one function, informational, target-scoped. |
| `analyze effects` | **`view effects`** | single-file effect listing, informational. |
| `analyze exceptions` | **`view exceptions`** | single-file exception-flow listing. |
| `analyze architecture` | **`view graph`** *(MERGE — struct-union)* | `ArchitectureReport` (cross-imports, hubs, coupling-hotspots, cycles) and `GraphReport` (sccs, bridges, centrality) are the **same whole-import-graph domain**. Merge into one report carrying cycles+hubs+coupling-pairs+centrality. This is the **one sanctioned merge**; it is a genuine struct-union, not a flag-selects-one-of-two-structs enum-wrap. |
| `analyze coupling-clusters` | **`view coupling-clusters`** | `CouplingClustersReport` = connected-component groups (graph) — different shape from `rank coupling` (ranked pairs). Kept separate; cross-reference both ways (M-6). |
| `analyze repo-coupling` | **`view repo-coupling`** | `RepoCouplingReport` = dep_edges + temporal_pairs + repos = a (cross-repo) graph. |
| `analyze activity` | **`trend activity`** *(soft fit — FLAG §8)* | `ActivityReport` = commit activity over time windows = temporal. Closest shape is time-series; not a clean fit (it is windowed counts, not metric-over-commits). Flagged. |
| `analyze cross-repo-health` | **`rank cross-repo-health`** | see §3.1 (ranked repos). |
| `analyze skeleton-diff` | **`view skeleton-diff`** | `SkeletonDiffReport` = structural delta between two refs = a single comparison artifact → inspect under `view`. |

After this, the `analyze` service/verb **no longer exists**.

### 3.3 `view` — keep verb, gains the graph/tree/dataflow/diff families

| Today | New home | Note |
|-------|----------|------|
| `view` (default target) / `chunk` / `list` / `references` / `referenced-by` / `dependents` / `import-path` / `history` / `blame` / `trace` / `graph` | **unchanged** (regrouped into topics §2) | — |
| *(gains)* `view cfg` | from `cfg cfg` | §3.6 |
| *(gains)* `view size` | from `rank size` | §3.1 |
| *(gains)* `view liveness / effects / exceptions / coupling-clusters / repo-coupling / skeleton-diff` | from `analyze` | §3.2 |
| `view graph` | **absorbs `analyze architecture`** (struct-union) | §3.2 |

### 3.4 `check` — NEW verb (cross-cutting verdict aggregator)

`check` is a **facade service in the main `normalize` crate** that mounts verdict
entry-points delegating into the owning crates (`normalize_budget`, `normalize_ratchet`,
`normalize_rules`). This is legitimate cross-cutting CLI wiring (command dispatch), the
one category CLAUDE.md says lives in the main crate. The verdict report structs stay in
their owning crates.

| Today | New home | Note |
|-------|----------|------|
| `ci` | **`check`** (bare = run all gates) | absorbs ci; `ci` retained as a transitional alias (§5). |
| `budget check` | **`check budget`** | CRUD (`add/show/update/remove`) + `measure` stay under `budget`. |
| `ratchet check` | **`check ratchet`** | CRUD + `measure` stay under `ratchet`. |
| `rules run` | **`check rules`** | rule-engine verdict moves to `check`; ruleset management stays under `rules`. |
| `rules run --fix` | **`check rules --fix`** | straddler decision below. |
| *(gains)* `check security` / `check docs` | from `analyze` | §3.2 |

**Straddler — `rules run --fix` (diagnostic + mutation):** lands as a **flag on
`check rules` (`check rules --fix`)**, not a separate `edit` command. Justification: the
*intent* is enforcement (run the rules); applying autofixes is an output projection of
the same findings, gated behind an explicit `--fix` (default is report-only, satisfying
the dry-run-by-default hard constraint — analogous to `edit extract-function --apply`).
It is not an enum-wrap: one report, one operation, an optional action on its findings.
Cross-reference from `edit`.

### 3.5 `trend` — keep verb

| Today | New home |
|-------|----------|
| `trend multi / complexity / length / density / test-ratio` | **unchanged** |
| *(gains)* `trend activity` | from `analyze` (soft — §8) |

### 3.6 `overview` — NEW verb (aggregate/dashboard home)

`overview health`, `overview summary`, `overview all` (from `analyze`). **Verb name is
an open question (§8).** Note: `analyze all` ≈ `analyze health` today (M-4); consider
collapsing `all` into a `--full` flag on `overview health` during implementation rather
than carrying three near-identical dashboards — flag for the human.

### 3.7 `cfg` — DISSOLVED

| Today | New home | Note |
|-------|----------|------|
| `cfg cfg <fn>` | **`view cfg <fn>`** | single-function control-flow graph = single-target inspection. `normalize_cfg::service::CfgService` is unmounted as a top-level verb; its method re-mounts under `view` (H-1, T2-7). |

### 3.8 `edit` — keep verb, one rename

| Today | New home | Note |
|-------|----------|------|
| `edit history *` | **`edit log *`** (rename) | resolves the `view history` collision (M-3, T2-10). `HistoryService` → mounted as `log`. |
| all other `edit *` | **unchanged** | — |

### 3.9 All other services — unchanged

`grep`, `init`, `structure`, `syntax` (apply M-5 `--compact`→`--outline` separately),
`config`, `daemon`, `serve`, `grammars`, `generate`, `package`, `sessions`, `context`,
`tools`, `guide`, `update`, `aliases`, `translate`, `docs`, `sync`, `kg` (apply H-3
stale-examples fix separately), `budget` (CRUD+measure), `ratchet` (CRUD+measure),
`rules` (list/show/tags/enable/disable/add/update/remove/setup/validate/compile/test/
test-fixtures — everything except `run`).

### 3.10 Contested-command resolution summary

| Contested item | Resolution | Justification |
|----------------|------------|---------------|
| All `analyze/*` | dissolved → §3.2 | shape-routed; verb retired |
| `rank budget` | → `rank purposes` (rename) | frees the word; stays rank (has `RankEntry`) |
| `rank size` (tree) | → `view size` | tree, no `RankEntry`; lint forces it out of rank |
| `cfg cfg` | → `view cfg` | single-target graph |
| `edit history` | → `edit log` | name collision with `view history` |
| `view graph` ↔ `analyze architecture` | **MERGE** (struct-union under `view graph`) | same whole-graph domain; sanctioned |
| `rank coupling` ↔ `analyze coupling-clusters` | **KEEP SEPARATE** (`rank coupling` pairs / `view coupling-clusters` graph) + cross-ref | different shapes; merging = banned enum-wrap |
| `rules run --fix` | `check rules --fix` (flag) | verdict intent; fix is projection |
| `trend` | keep verb; gains `trend activity` (soft) | time-series |
| `kg` | keep (fix stale `--help`) | distinct data domain |
| `syntax` | keep (fix `--compact`) | distinct CST domain |
| aggregate (`health`/`summary`/`all`) | `overview` (name OPEN) | bounded dashboard special-case |

---

## 4. CI lint spec

**What it checks:** each `#[cli]` method's mount verb must match its return-type shape.

**Mechanism (compile-time-first, strongest guarantee):** introduce shape marker traits
and bound the mount methods so a mismatch fails to **compile** ("impossible by
construction" rather than "caught at CI"):

```rust
// crates/normalize-analyze/src/ranked.rs (alongside RankEntry)
pub trait RankedReport {            // a report that IS a ranked list
    type Entry: RankEntry;
    fn entries(&self) -> &[Self::Entry];
}
pub trait Verdict {                 // a pass/fail diagnostic
    fn passed(&self) -> bool;
    fn exit_code(&self) -> i32;
}
pub trait TimeSeriesReport { /* points + delta */ }
```

- Every method mounted under **`rank`** returns `R: RankedReport`. Mounting a
  non-ranked report fails to compile. (This is the strong half — it covers the exact
  seam that broke.)
- Every method under **`check`** returns `R: Verdict`.
- Every method under **`trend`** returns `R: TimeSeriesReport`.
- **`view`** and **`overview`**: exclusionary — a test (or a `#[deny]`-style negative
  bound via a sealed marker) asserts their return types do **not** impl `RankedReport`
  or `Verdict`. Catches drift back toward rank/check.

**Fallback (if a compile-time bound is too invasive across crates):** an `xtask`/native
rule that parses the `#[cli]` service impls (or the generated `cli-snapshot`) + a
hand-maintained "return type → trait impls" map, run in CI. Weaker (it can lag the
code) but still converts drift to a build failure. Prefer the trait-bound approach.

**Known cases the lint MUST flag (regression guards):**
- `rank size` → `SizeReport` is not a `RankedReport` (today's live miscategorization).
  After the retree this is resolved by the move; the lint guarantees it can't return.
- A future `analyze`-style "unordered report" mounted under `rank` → flagged.

**Known cases the lint MUST allow:**
- All current `rank` members except `size` (each has a `Vec<T: RankEntry>` primary —
  31 entry types impl `RankEntry`, verified).
- `rank cross-repo-health` once `RepoHealthEntry: RankEntry` is added.

---

## 5. Transitional alias plan

**Finding (verified in server-less 2026-06-29):** server-less `#[cli]` supports
`#[server(hidden)]` (mounts the method but hides it from help/docs) and `#[cli(name=…)]`
(rename a mount), but has **NO `#[cli(alias=…)]`** attribute. clap underneath supports
hidden `.alias()` / visible `.visible_alias()`, so the capability exists but is not
surfaced.

**Mechanism (two prerequisites + one policy):**

1. **Add `#[cli(alias = "...")]` (and `aliases = [...]`) to server-less**, mapping to
   clap's hidden `.alias()`. This is a server-less feature (fix it there per CLAUDE.md,
   don't work around it here). Covers **same-parent renames**: `rank budget`→
   `rank purposes`, `edit history`→`edit log`, `cfg cfg`→`view cfg` (same leaf word
   moving parents needs #2).
2. **Cross-verb moves keep the OLD parent mounted as a hidden shim for one release.**
   clap aliases are scoped to one parent, so `normalize analyze complexity` cannot be a
   hidden alias of `normalize rank complexity` from under `rank`. For the transitional
   release, keep a `#[server(hidden)]` `analyze`/`ci`/`budget check`/`ratchet check`/
   `rules run` mount whose body delegates to the new home (or prints a one-line "moved
   to `X`; this alias is removed at 1.0" to stderr and runs it). These hidden shims are
   migration scaffolding.
3. **Policy:** every renamed/moved command keeps its hidden old path for **exactly one
   release**. At 1.0 the alias attrs and the hidden shim services are **deleted**. This
   satisfies "retire, don't deprecate" (the ban is on *permanent* compat cruft; the repo
   already endorses one-release hidden aliases — see T2-3's `--base-ref` verdict).

**`ci`:** `check` (bare) becomes the canonical "run all gates"; `ci` is retained as a
transitional alias for one release (CI ergonomics), removed at 1.0. (If the human wants
`ci` permanent for pipeline muscle-memory, that's an §8 naming call — but the default is
retire.)

---

## 6. Guide / help regression test spec

Closes T1-6's deferred follow-up and is the **gate that must land FIRST** (before any
rename) so the retree can't re-break guides the way H-4/H-5 did.

**Test:** for each `guide` body (`rules`, `explore`, `setup`, `analyze`/renamed,
`tree-sitter`) and each `--help`/`--manual` cross-reference:
1. Regex-extract every line matching `^\s*normalize <args>` (the command examples).
2. For each, resolve the command path against the live CLI command tree (parse with the
   server-less app in a dry/`--help` mode, or check the generated `cli-snapshot`), and
   assert it resolves — no "unrecognized subcommand".
3. Fail the test (and CI) on any unresolved example.

**Implementation note:** guide bodies are `const &str` in `service/guide.rs`; the harness
is "extract command lines + smoke-resolve each." Pair it with the topic-snapshot test
(§2) so the second-level grouping is also covered. After the retree, every `guide analyze`
example must be rewritten to its new home in the **same commit** (the doc-sync rule).

---

## 7. Migration execution plan

**Blast radius: ~22 command invocation strings change of ~165 (~13%)** — the
B-shaped retree (human-named). Breakdown: `analyze` 14, `rank` 2 (size, budget),
`cfg` 1, `budget check` 1, `ratchet check` 1, `rules run` 1, `ci` 1, `edit history` 1.

**Order (each step keeps build + `cargo test -q` green):**

0. **Land §6 guide/help regression test + §2 topic-snapshot test FIRST.** Then land the
   §4 shape marker traits (`RankedReport`/`Verdict`/`TimeSeriesReport`) and the lint with
   the *current* tree green (this surfaces `rank size` as the one expected failure, which
   step 2 fixes).
1. **server-less prerequisite:** add `#[cli(alias=…)]` (§5). Land + publish (it's a
   separate repo; bump the dep). Without it, same-parent renames have no clean alias.
2. **Per-verb batches, lowest-risk first** (each batch = mounts + report-trait impls +
   docs + snapshot regen in one commit, with hidden aliases):
   - **B1 `view` gains:** `cfg cfg`→`view cfg`; `rank size`→`view size`; the
     `analyze` dataflow/graph/diff members (`liveness/effects/exceptions/
     coupling-clusters/repo-coupling/skeleton-diff`)→`view`; **`view graph` ←
     `analyze architecture` struct-union merge** (the only real code-merge in the plan).
   - **B2 `rank`:** `rank budget`→`rank purposes`; `analyze cross-repo-health`→
     `rank cross-repo-health` (+ `RepoHealthEntry: RankEntry`).
   - **B3 `trend`:** `analyze activity`→`trend activity`.
   - **B4 `overview`:** new verb; `analyze health/summary/all`→`overview` (decide
     `all`-collapse per §8).
   - **B5 `check`:** new facade verb; `ci`→bare `check`; `budget check`/`ratchet check`/
     `rules run`→`check budget/ratchet/rules` (+ `check rules --fix`); `analyze
     security/docs`→`check security/docs`.
   - **B6 retire `analyze`:** verb now empty → delete service; convert remaining old
     paths to hidden shims (one release).
   - **B7 `edit`:** `edit history`→`edit log`.
3. **Final sweep:** regenerate `cli-snapshot`; run guide/topic snapshot tests; full
   `cargo clippy --all-targets --all-features -- -D warnings && cargo test -q`.

**Doc sync (same commits as the code, per the hard rule):** `docs/cli/`, `README.md`,
`LLMS.md`, `docs/cli-design.md`, all `guide` bodies, `CHANGELOG.md` (`[Unreleased]`),
and every directory's `SUMMARY.md` touched (notably `crates/normalize/src/service/` and
`commands/analyze/` if structs relocate). **Report structs do NOT need to move** — the
mounts (`#[cli]` methods) move; the structs stay in their owning crates/modules
(`commands/analyze/*` keep their files even when mounted under `view`/`check`). This
keeps the per-batch diff small and the `assert_output_formatter` tests low-touch.

---

## 8. Open questions for the human (naming / taste calls)

Confirm before implementation:

1. **The aggregate/dashboard verb name.** Proposed: **`overview`**. Alternatives:
   `report`, `dashboard`, `audit` (clashes with `package audit`), `health`, `summary`.
   And: **collapse `analyze all` into `overview --full`** (it's ≈ `analyze health`, M-4)
   rather than carrying `overview all`?
2. **`check` (bare) vs keeping `ci`.** Proposed: bare `check` runs all gates; `ci` is a
   one-release transitional alias then removed. Alternative: keep `ci` as a *permanent*
   documented alias for pipeline ergonomics (would be a sanctioned exception to
   retire-don't-deprecate). Confirm: retire `ci`, or keep it?
3. **`rules run` → `check rules`.** This splits the well-regarded `rules` service
   (verdict→`check`, management stays). Candidate C argued *against* moving it (keep
   `rules run` top-level). The full-retree decision favors one verdict home (`check`).
   Confirm we move `rules run`→`check rules` (with `rules run` as a one-release alias),
   vs. the documented-exception of leaving `rules run` in place.
4. **`view` as the home for whole-graph/tree reports.** `view graph`, `view size`,
   `view coupling-clusters`, `view repo-coupling` stretch "view = one named target" to
   "view = one derived structure." Confirm this reading (the alternative — a `graph`
   top-level verb — was rejected on usability, but it's a genuine taste call).

### Flagged soft spots (shape-axis produces an imperfect result — NOT forced)

- **`trend activity`** (§3.5): `ActivityReport` is windowed commit counts, not
  metric-over-commits. It's the closest shape but not clean. *Option:* leave it as a
  flagged soft fit under `trend`, or move to `overview`/`view`. Recommend `trend` with a
  note; raise if the human prefers otherwise.
- **The multi-repo trio** (`activity`, `repo-coupling`, `cross-repo-health`): the shape
  axis scatters them across `trend`/`view`/`rank`, losing the "fleet" cohesion that
  candidate D's (rejected) axis captured. Only 3 low-traffic commands. *Flagged, not
  forced.* If the human wants them grouped, a documented `--repos`/fleet topic or a small
  `fleet` domain verb is the alternative — but that reintroduces a scope axis. Default:
  shape-route as mapped, cross-reference them in help.
- **`check docs`**: `DocCoverageReport` is a coverage dashboard with an *embedded* ranked
  Vec. Routed to `check` (coverage diagnostic). Honest ambiguity — it could read as
  `overview docs` or `rank docs`. The lint won't force it (primary field is a scalar).
  Recommend `check docs`; raise if the human reads it as ranking.

No command in the mapping is genuinely homeless and no enum-wrap was forced. The two
real soft spots (multi-repo trio, `trend activity`) are flagged above rather than forced.
