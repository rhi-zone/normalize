# Command-Surface Decomposition Audit — 2026-07-03

**Supersedes the 2026-07-02 audit's "~62k of own code mostly legitimately stays"
framing.** See `docs/audit-2026-07-02.md`. That audit applied the right lens for the
wrong question: it asked *"should this ALGORITHM be its own crate?"* and correctly found
that the reusable algorithms already live in feature crates. It did **not** apply
CLAUDE.md's other, orthogonal rule:

> *"a crate that owns a subcommand includes its own `#[cli]` service, report structs, and
> `OutputFormatter` impls. The main `normalize` crate just mounts them."*

Under **that** rule the question is not "is the algorithm reusable?" but "does the command
*surface* (its `#[cli]` service method + report structs + `OutputFormatter` impls) belong
in the crate that owns the feature?" A three-agent structural audit (2026-07-03) found the
answer is *yes* for a large fraction of the main crate. The command surface is
**substantially migratable** even though the algorithms are already extracted.

## Executive Summary

- The main `normalize` crate is **~84k lines**.
- **~21k** of that is vendored third-party CLI front-ends (`src/rg/`, `src/ast_grep/`,
  `src/jq/`). Those are **forced to stay** — see the publish-trilemma decision in
  `docs/audit-2026-07-02.md` ("Decision (2026-07-02): keeping the vendored CLIs in main is
  FORCED by a publish trilemma"). Not a decomposition target.
- Of the **~62k of normalize's OWN code**, roughly **~50k is migratable in principle** to
  feature crates under the "crate owns its subcommand, main just mounts" rule.
- **Realistic main-crate floor ≈ 30–34k total**, of which **~21k is vendored** → normalize's
  own irreducible core ≈ **9–13k** (command dispatch, global flags, output backend,
  aggregators, multi-repo, trend, init/update/sync, daemon, service composition, and the
  `view` family unless its internal tree/skeleton/parsers are extracted first).

This is **not hypothetical.** The migration pattern is already in production.

## The namespace break: decomposition ≡ taxonomy inversion

**Pivotal finding (2026-07-03, after the sessions migration landed in `2ca71235`):** this
effort is not a size-reduction chore that happens to touch the CLI — it *is* the
long-deferred CLI taxonomy inversion (B2–B12). The two are the same operation, reached
organically from the opposite direction (size concern rather than taxonomy design).

The discovery, step by step:

1. **One mounted crate = one top-level CLI command namespace.** In this codebase each
   mounted `#[cli]` service maps to exactly one top-level command: the accessor name *is*
   the command (`rank()`→`rank`, `budget()`→`budget`, `sessions()`→`sessions`). There is no
   mechanism for a mounted crate to contribute commands into some *other* namespace.

2. **Sessions migrated cleanly ONLY because `sessions` was already its own namespace.** The
   `sessions` surface was already a standalone service occupying its own top-level namespace,
   so moving it into `normalize-sessions` preserved every CLI path (`normalize sessions …`
   unchanged). That is why it was the frictionless proof case — and why its cleanliness does
   **not** generalize to the remaining families.

3. **Migrating commands that currently live under a SHARED namespace changes their CLI
   path.** `duplicates`/`fragments` are 2 of 24 methods on the shared `RankService`
   (namespace `rank`). Moving them into a mounted per-domain crate necessarily moves them
   *out of* `rank`: `normalize rank duplicates` becomes `normalize <new-namespace>
   duplicates`. This is a **user-facing breaking change** and forces a **per-domain naming
   decision**. It applies to *most* remaining families, because `analyze`/`rank` are grab-bag
   namespaces holding commands from many different domains.

4. **Therefore: command-surface decomposition ≡ the CLI taxonomy inversion.** Decomposing
   command surfaces into per-domain crates and reorganizing the CLI taxonomy so each domain
   owns its namespace are **the same operation**. This reconnects to the long-deferred
   B2–B12 "CLI taxonomy inversion" thread — see `docs/cli-design.md`, the
   `docs/artifacts/cli-taxonomy-2026-06-29/` artifacts (esp. `00-inversion-plan.md` and
   `seam-evaluation.md`), and the TODO "CLI command-taxonomy FULL INVERSION" thread. We
   arrived at that thread from the size-concern direction.

### DECISION (user-approved, 2026-07-03): FULL MOVE = taxonomy inversion

The roadmap proceeds by moving the **whole command surface (including the `#[cli]` service)**
into per-domain crates, **accepting the CLI path changes**, to fully reach "main just mounts"
and the ~30k floor — and thereby *executing* the taxonomy inversion. This is **not** the
non-breaking half-move (keeping the `#[cli]` service in main pointed at an extracted impl).
Full move is the decided path; breaking CLI paths under a coherent target taxonomy is
accepted.

### AUTHORITATIVE target taxonomy: the 2026-06-29 inversion plan (FINAL SCOPE)

**Do not redesign the taxonomy here.** It is already designed. The canonical target —
confirmed verb names, per-command owning crate, and a B0–B12 batch sequence — lives in
`docs/artifacts/cli-taxonomy-2026-06-29/00-inversion-plan.md` (the **FINAL SCOPE** section
is canonical; the body is supporting evidence), with seam evidence in the sibling
`seam-evaluation.md`. **This decomposition roadmap and that inversion plan are the SAME
operation reached from opposite directions** (size-reduction vs taxonomy design). This
document's job is to *reconcile* with the inversion plan and record the residual open
forks — not to duplicate or re-derive the target. Where the two disagree, the migration map
below is corrected to match the inversion plan's command→crate ground truth (traced by
reading each module), and genuine unreconciled points are listed under **Open forks** below.

### Consequence for sequencing: design the taxonomy BEFORE the next migration

Family-by-family migration with *improvised* namespaces would yield an incoherent CLI (each
family inventing its own top-level verb ad hoc). So the **next step is NOT another family
migration.** It is to **design the coherent target taxonomy first**: the full set of
top-level namespaces, how the ~40 `analyze`/`rank` commands (plus others) map to domain
namespaces, and which crate owns each. That design is grounded in `docs/cli-design.md`,
the B2–B12 inversion plan, and this decomposition roadmap. Migrations then execute *into*
the designed taxonomy. **Code-similarity and every other pending family are blocked on the
taxonomy design.**

## The pattern is already in production (evidence)

Five sub-services already follow the "crate owns its subcommand, main just mounts" rule
today, each with a `#[cli]` service + report structs + `OutputFormatter` impls and **zero
back-references into the main crate**:

- `normalize-budget`
- `normalize-cfg`
- `normalize-ratchet`
- `normalize-rules`
- `normalize-knowledge-graph`

`service/mod.rs` mounts each in **one line** of state plus a constructor line and an
accessor — the field, the `Service::new()` wiring, and the getter. The migration target for
every row in the map below is exactly this shape: move the implementation into the owning
crate, expose a `#[cli]` service there, and reduce main to a mount.

## OutputFormatter is NOT a blocker

A recurring objection — "the report structs implement `OutputFormatter`, which lives in
main, so they can't move" — is **false**. `OutputFormatter` is defined in the standalone
`normalize-output` crate (`crates/normalize-output/src/lib.rs:94`).
`crates/normalize/src/output.rs` is just `pub use normalize_output::*` — a re-export for
in-crate ergonomics, not the definition. Feature crates implement `OutputFormatter`
standalone today (all five mounted crates above do). A migrating report struct depends on
`normalize-output` directly and keeps its `format_text()` / `format_pretty()` impls
verbatim. No coupling to main.

## The real blocker is ORDERING

The movable per-feature `service/*.rs` methods do **not** contain the implementation — they
**delegate into the monolithic in-main `crate::commands` module** (171 references). So
extracting a `#[cli]` service method is **downstream** of extracting the `commands/`
implementation it calls. The correct order per feature is:

1. Move the `commands/<feature>/` implementation into the feature crate.
2. *Then* the thin `#[cli]` service method follows it (or is recreated in the crate), and
   main drops to a mount.

Attempting to move the service method first fails because it still points at
`crate::commands::…`.

## Two small enablers unblock the analyze-family migrations

The analyze-family commands share two couplings to main. Both are small and both have an
established precedent for how to break them:

1. **The daemon-aware `crate::index` wrapper** (~144 LOC over `normalize_facts::FileIndex`)
   adds auto-rebuild + daemon awareness. Migrating crates either use
   `normalize_facts::FileIndex` **directly** (as `normalize-ratchet` and `normalize-budget`
   already do), or `index.rs` is hoisted into a small shared crate. Direct use is the
   lighter path and matches existing consumers.

2. **`crate::config::NormalizeConfig` per-subcommand excludes.** Several analyze commands
   read their exclude globs from the main config. The fix is to pass each migrating crate
   its **config slice** (the excludes it needs) rather than the whole `NormalizeConfig`.

Neither is a large piece of work; both are prerequisites for the code-similarity /
architecture / graph / cfg family.

## Migration map

| Target | ~LOC | Owning crate | Notes |
|---|---|---|---|
| Sessions (`commands/sessions/` + `service/sessions.rs`) | ~8k | **NEW** `normalize-sessions` (deps: `normalize-chat-sessions` + `normalize-session-analysis`) | Cleanest — near-zero coupling (only `crate::output` re-export + `super::` internals + `resolve_pretty` rewire). **DO FIRST.** |
| duplicates / duplicate-types / fragments | ~3k | `normalize-code-similarity` (already the compute dep) | Gated on index/config enablers. **Correction:** `clusters`/`coupling_clusters` are NOT here — they are git-temporal (see history row). Code-similarity's family is exactly these three (→ inversion-plan `similarity` verb, B4). |
| architecture / layering / depth_map | ~0.9k | `normalize-architecture` | Gated on index/config enablers. → inversion-plan `architecture` verb (B3). |
| graph / dependents / import-path | ~1.2k | `normalize-graph` | **Correction:** these are `view` subcommands today (`view graph` / `view dependents` / `view import-path`), NOT `analyze`/`rank` — the `normalize-graph` verb carves out of **`view`**, not analyze/rank. (`view references`/`referenced-by`/`trace`/`blame` are separate view leaves that stay main.) → inversion-plan `graph` verb (B2). Gated on enablers. |
| clusters / coupling_clusters (git co-change) | ~0.5k | `normalize-git-history` (B8/B9 `history` verb) | **Correction (de-double-claim):** git-temporal — "files that change together in git history", via daemon-aware `crate::index` + `co_change_edges`. Was claimed by code-similarity above; it belongs with the git-history family, not clone detection. |
| liveness / effects / exceptions (dataflow trio) | ~0.9k | **✅ EXECUTED (B5, 2026-07-03) → `normalize-facts`, `structure` verb** | Code home FORCED to facts (they read `cfg_*` tables via `idx.connection()`; `normalize-cfg` would create a `facts ⇄ cfg` compile cycle). Now `structure liveness`/`effects`/`exceptions`; old `analyze` paths hidden shims for one release. |
| provenance | ~0.75k | `normalize-chat-sessions` / `normalize-session-analysis` | `view provenance` leaf. — |
| small wrappers (generate / context / package / find_references service + report surfaces) | ~2k | `normalize-typegen` / `-context` / `-ecosystems` / `-scope` | Follows the budget template. |
| rank-style metrics (hotspots, contributors, ownership, density, ceremony, test_ratio, call_complexity, size, docs, coupling, imports, uniqueness, module_health, surface, complexity/length/test_gaps, budget-metric) | ~5.7k | **NO owner today — DECISION NEEDED:** designate `normalize-metrics` as home, or they stay | **Not** blocked by `OutputFormatter`; blocked by the **absence of a home**. |
| security | small | **NO home in any map — genuinely unassigned** | `analyze security`. Neither this roadmap nor the inversion plan assigns it a compute crate. Candidate: a future security crate, or stays main. See Open forks. |
| skeleton-diff | small | none — view/skeleton family | `analyze skeleton-diff` composes the internal skeleton extractor; **stays main** unless tree/skeleton/parsers are extracted (same gate as `view`). |
| docs (coverage) | small | none — main residual | `analyze docs`. Part of the ~19-subcommand metrics bucket with no owning crate (see Open forks #1). |
| view (`commands/view/` + tree / skeleton / parsers) | ~3.5k | none — intrinsically main (composes internal tree/skeleton/parsers) | **STAYS** unless tree/skeleton/parsers are extracted first. |
| `service/edit.rs`, `service/facts.rs` | large | edit/facts crates exist but the service is coupled to `crate::index` / shadow state | Later. |
| aggregators (report / summary / mod `AnalyzeConfig`), multi-repo (activity / repo_coupling / cross_repo_health), trend, init/update/sync, daemon, service composition | ~irreducible | — | Genuinely stays in main. |

## Open forks (must be resolved before/at execution)

These are the points where this roadmap and the inversion plan either disagree or leave a
command genuinely unassigned. They block execution of the affected batches — resolve each
before the batch that touches it. Everything else is settled by the inversion plan's FINAL
SCOPE.

1. **Metrics bucket home (A1 vs A2) — the biggest open decision.** ~19 subcommands (most of
   `rank`: complexity/length/ceremony/density/uniqueness/imports/surface/module-health/size/
   files/test-ratio/test-gaps/call-complexity, `rank budget`/purposes, plus `analyze docs`
   coverage) have **no owning compute crate**.
   - **A1** — keep `rank`/`trend` permanently main-resident. The inversion-plan seam
     evaluation (`seam-evaluation.md §Candidate 1`) *recommends A1*: the metrics are two
     disjoint dependency groups (AST-group vs index-group), not a coherent domain; a
     `normalize-metrics`-AST crate would collide with the existing ratchet `normalize-metrics`,
     have one dependent, duplicate `compute_complexity`, and exist solely to back a verb. A
     `RankEntry` CI lint (B11) holds against drift.
   - **A2** — extract a `normalize-metrics`-family crate and mount a `metrics` verb. Only
     path to dissolving the `rank` core by crate-ownership; large precondition phase.
   - **Status: UNRESOLVED.** The inversion plan closed this as A1; recorded here as still-open
     because it is the load-bearing architectural call and the human has not re-ratified it in
     the decomposition framing.

2. **Dataflow trio (`liveness`/`effects`/`exceptions`) home — RESOLVED (2026-07-03).**
   - **Physical code home: `normalize-facts` — FORCED (not a preference).** The three commands
     are index/table readers: they open `idx.connection()` and run libsql SELECTs against the
     `cfg_*` tables that `normalize-facts` owns (schema `index.rs:628`, writers `index.rs:3134+`,
     and the `cfg_dataflow` solver already living there since `b8e5da99`). Homing them in
     `normalize-cfg` (this roadmap's earlier suggestion) is architecturally **impossible**: it
     would force `cfg → facts + libsql`, but `facts → cfg` already exists
     (`normalize-facts/Cargo.toml:37`, used at `index.rs:4339`), creating a `facts ⇄ cfg`
     **compile cycle** — and it would contaminate the deliberately pure in-memory `normalize-cfg`
     and undo `b8e5da99`. The 07-03 roadmap's `normalize-cfg` suggestion is therefore superseded
     by the dependency-cycle finding.
   - **Verb label: `structure` (inversion-plan B5).** Commands become
     `normalize structure liveness/effects/exceptions`. Chosen for least machinery — code and
     verb both land in facts, zero new Cargo edges, matches the plan of record.
   - **Recorded ALTERNATIVE for B5-execution.** The conceded downside is that `structure liveness`
     reads grab-baggy (these are CFG analyses, not index introspection — the inversion plan's
     §Flagged soft spots concedes exactly this: they "read as analysis not index introspection").
     The clean alternative that yields the semantically-correct `cfg liveness` name is: have
     `normalize-facts` host a `cfg` verb by **also** moving the render command (currently
     `normalize-cfg`'s `CfgService`) into facts, making `normalize-cfg` a pure library
     (model + builder + mermaid, no CLI). This is permitted by the dep graph (`facts → cfg`, no
     cycle) but costs a render-move plus un-mounting cfg's service. **Reconsider this at
     B5-execution time if the `structure` naming grates — do not decide it now.**
   - **Status: RESOLVED** (code→facts forced; verb=`structure`; cfg-consolidation alternative
     parked for B5-execution).

3. **`search` verb collision (RESOLVED, 2026-07-03).** Inversion-plan **B7** wires
   `normalize-semantic` as a new top-level **`search`** verb (semantic code search). `search`
   currently exists as a user-facing **alias for `grep`**. **Decision (user-approved):** drop
   the `search`→`grep` alias and let `search` become the semantic verb. The alias removal is
   **executed at B7, atomically with mounting the `search` verb** (removing it earlier would
   delete a convenience with nothing replacing it until B7). The `find`→`grep` alias is
   unaffected and remains the grep-oriented shortcut. Until B7 the alias still stands in
   `main.rs`/`rewrite_aliases` and the `docs/cli-design.md` aliases table (row annotated as
   slated for removal at B7).

4. **`analyze security` home — genuinely unassigned.** No compute crate in either map.
   Candidate: a future security crate, or stays main. **Status: OPEN** (not blocking; parks
   under a slimmed `analyze` or `overview` until decided).

5. **`coupling-clusters` → history (RESOLVED here).** Recorded for the record: it was
   double-claimed by code-similarity; it is git-temporal and belongs with the
   `normalize-git-history` family (B8/B9). Migration map corrected above.

## Recommended order

1. **Sessions first** (~8k, no blockers) — proves the full migration for a large surface
   (a whole `commands/` subtree plus its `service/*.rs` method) end to end.
2. **Build the two enablers** — shareable index acquisition (direct `FileIndex` use or
   hoist `index.rs`); config excludes-slice.
3. **Analyze families to existing owners** (~7.75k): code-similarity, architecture, graph,
   cfg, chat-sessions/session-analysis.
4. **DECISION on the ~5.7k rank-metrics** — designate `normalize-metrics` as owner vs. leave
   them in main. This is the one genuinely open architectural call in the roadmap; the
   metrics have no home today and forcing one is a real decision, not a mechanical move.
5. **Small wrappers** (~2k) — generate / context / package / find_references, following the
   budget template.

## Relationship to prior audits

- **`docs/audit-2026-03-12.md`** (P2: "extract analysis algorithms individually"): satisfied
  — algorithms are in crates. That item was about algorithm reuse, which this audit does not
  disturb.
- **`docs/audit-2026-07-02.md`** (main-crate decomposition): correct on algorithms, but its
  conclusion that the ~62k of own code "mostly legitimately stays" was drawn through the
  crate-extraction-bar lens (does the *algorithm* deserve a crate?) and missed the
  command-surface-migration lever (does the *subcommand surface* belong with the feature that
  owns it?). This audit supersedes that specific framing. The 07-02 audit's algorithm
  findings (D1–D6 + the rename) and the vendored-CLI trilemma decision remain valid and are
  unaffected.
