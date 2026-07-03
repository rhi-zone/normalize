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
| duplicates / fragments / clusters / coupling_clusters | ~4k | `normalize-code-similarity` (already the compute dep) | Gated on index/config enablers. |
| architecture / layering / depth_map | ~0.9k | `normalize-architecture` | Gated on index/config enablers. |
| graph / call_graph | ~1.2k | `normalize-graph` | Gated on index/config enablers. |
| liveness / effects / exceptions | ~0.9k | `normalize-cfg` (CfgService already mounted) | Gated on index/config enablers. |
| provenance | ~0.75k | `normalize-chat-sessions` / `normalize-session-analysis` | — |
| small wrappers (generate / context / package / find_references service + report surfaces) | ~2k | `normalize-typegen` / `-context` / `-ecosystems` / `-scope` | Follows the budget template. |
| rank-style metrics (hotspots, contributors, ownership, density, ceremony, test_ratio, call_complexity, size, docs, coupling, imports, uniqueness, module_health, surface, complexity/length/test_gaps, budget-metric) | ~5.7k | **NO owner today — DECISION NEEDED:** designate `normalize-metrics` as home, or they stay | **Not** blocked by `OutputFormatter`; blocked by the **absence of a home**. |
| view (`commands/view/` + tree / skeleton / parsers) | ~3.5k | none — intrinsically main (composes internal tree/skeleton/parsers) | **STAYS** unless tree/skeleton/parsers are extracted first. |
| `service/edit.rs`, `service/facts.rs` | large | edit/facts crates exist but the service is coupled to `crate::index` / shadow state | Later. |
| aggregators (report / summary / mod `AnalyzeConfig`), multi-repo (activity / repo_coupling / cross_repo_health), trend, init/update/sync, daemon, service composition | ~irreducible | — | Genuinely stays in main. |

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
