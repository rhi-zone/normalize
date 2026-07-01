# Decision Record: normalize-graph Crate Architecture

**Decided:** 2026-07-01
**Resolved:** 2026-07-02
**Status:** RESOLVED — refactored in place (no standalone crate; no node-type genericization)
**Evidence artifacts:** `graph-crate-reality.md`, `graph-crate-justification.md`

---

## Resolution (2026-07-02) — supersedes the "separate the halves / defer" framing below

A petgraph/ecosystem capability survey closed the question the opposite way to the
original "spin the generic half into its own crate" plan:

- **No plumbing gap over the existing ecosystem.** petgraph's `Visitable`/`VisitMap` +
  trait-generic algorithms already provide bring-your-own-node-type with optimal per-type
  visit maps; the `pathfinding` crate provides a ~5-line closure-based SCC directly over raw
  `HashMap`s. There is no missing "generic graph-algorithms crate" for normalize to build.
- **`find_longest_chains` is an admitted APPROXIMATE, normalize-flavored heuristic** — a
  cycle-tolerant longest-*simple*-path DFS with node-keyed memoization and suffix-dominance
  dedup, whose through-cycle output is order-dependent by design. It is *not* a correct
  longest-path algorithm (correct DAG-longest-path already exists in rustworkx-core). It
  stays internal; extracting it would advertise a correctness it does not have.
- **Motif/diamond detection IS a genuine ecosystem gap** (only exact VF2 subgraph
  isomorphism exists in Rust). It *could* be a worthwhile INDEPENDENT library — but it is
  deliberately NOT extracted from normalize and NOT made a normalize dependency: the coupling
  cost of an external dep for one ~50-line algorithm dwarfs the benefit. normalize keeps its
  own `find_diamonds`.

**Resolution: refactor `normalize-graph` in place.** Split pure algorithms from
presentation (report structs, `GraphTarget`, `OutputFormatter` impls, `assemble_graph_report`
moved to `crates/normalize/src/commands/analyze/graph.rs`); `normalize-graph` drops its
`normalize-output` and `nu-ansi-term` deps and is now genuinely "pure algorithms, no
normalize types"; the dead duplicate `find_longest_chains` was already removed from
`normalize-architecture`; a `#[cfg(test)]` characterization suite now pins behavior.

**Node-type genericization is DROPPED.** Every caller uses `String`; the datatype-agnostic
goal only served the abandoned standalone-crate ambition. No standalone crate will be built.

The refactor also **surfaced a pre-existing bug**: the iterative `tarjan_sccs` runs its
SCC-root check before children update `lowlink` (sentinel push-order), so it returns all
singletons and `find_sccs` never reports a circular-dependency cluster. Characterized (pinned)
but not fixed here; tracked in `TODO.md`.

The material below is the ORIGINAL 2026-07-01 record, retained for provenance. Where it says
"separate the generic half into its own crate" / "genericize over node type" / "DEFERRED",
read it as superseded by the resolution above.

---

## The Decision

The generic graph algorithms in `normalize-graph` are NOT normalize-flavored — they operate
on `HashMap<String, HashSet<String>>` with no semantic entanglement with the import/call/type
domain. They do not belong in normalize. **Separate them.**

The user has green-lit (in principle) spinning the generic half out to its own crate/repo if
it earns it. The normalize-flavored half stays in normalize, moved to the crate(s) that
actually own the domain.

---

## Evidence Summary

**What the investigation found:**

- **petgraph is absent** from the workspace — zero external graph crate dependencies across
  all `crates/*/Cargo.toml`. All algorithms are hand-rolled over `std::collections`. petgraph
  is a poor fit anyway: it uses typed `NodeIndex`/`EdgeIndex` handles, not adjacency lists;
  bridging normalize's `HashMap<String, HashSet<String>>` to petgraph's type would require
  non-trivial conversion overhead and API surface. Adding petgraph (~20K LOC) to avoid
  maintaining ~500 LOC of hand-rolled algorithms is a questionable trade on dep-weight grounds.

- **The niche for minimal-interface algorithms is real but partly contested.** Crates like
  `pathfinding` serve some of the same space. Standalone-usefulness is plausible but not
  certain — depends on positioning and API design.

- **Algorithms are String-pinned, not semantically entangled.** Every public function
  signature takes `HashMap<String, HashSet<String>>` — no module-name parsing, no DB calls,
  no normalize types. Genericizing over `<N: Hash + Eq + Clone + Ord>` is mechanical but
  non-trivial (every `.to_string()` → `.clone()`, every `&str`-keyed lookup → `&N`).

- **Several algorithms have no petgraph equivalent:** diamond detection, longest-path with
  suffix dedup (cycle-avoiding memoized BFS), and the directional bridge filter. These
  represent genuine standalone value that petgraph does not cover.

- **Exactly 2 workspace consumers today:** `normalize` (main crate, uses the full analysis
  surface) and `normalize-architecture` (thin — imports only `ImportChain` as a type, then
  reimplements `find_longest_chains`/`longest_path_from` independently). The latter is an
  outright duplication bug: `normalize-architecture` re-exports `ImportChain` from
  `normalize-graph` while reimplementing the function that produces it with slightly different
  thresholds.

- **~60% generic algorithm bodies / ~40% normalize-specific** report shaping: `GraphTarget`
  (Modules/Symbols/Types enum), `DependentEntry.has_tests`/`fan_in` enrichment from the
  index, `BlastRadius`, `GraphReport`/`DependentsReport`, all `OutputFormatter` impls.
  The normalize-specific pieces have no place in a standalone graph library.

- **The crate's own module doc is false:** it claims "No filesystem, CLI, or
  normalize-specific types" — in reality `GraphTarget`, `DependentEntry.has_tests`,
  `BlastRadius`, and the `OutputFormatter` impls are all normalize-specific.

---

## The Clean Boundary (shape to implement when this is executed)

### Generic half → standalone graph-algorithm functions

Datatype-agnostic functions generic over node type and a minimal "neighbors" interface
(closure or iterator — bring-your-own-representation). NOT a `Graph<N, E>` container; that
stays distinct from petgraph's approach and matches normalize's adjacency-list usage.

Self-contained, zero normalize dependencies. Includes exactly the algorithms petgraph lacks
and that represent the genuine standalone value:

- `find_diamonds` — diamond pattern detection
- `find_longest_chains` / `longest_path_from` — longest path with suffix dedup and
  cycle-avoiding memoization
- Directional bridge filter (`find_bridges` directional variant)

Plus the algorithms petgraph does cover (SCC, BFS, WCC) — kept for the zero-dep adjacency-
list API that avoids petgraph's index-handle overhead.

### Normalize-flavored half → moves to the dependency-analysis callers

- `GraphTarget` (Modules/Symbols/Types) → wherever the dependency analysis command lives
- `DependentEntry.has_tests`/`fan_in` enrichment → `normalize-facts` (owns the index, owns
  the enrichment)
- `BlastRadius` → dependency-analysis caller
- `GraphReport`, `DependentsReport`, all `OutputFormatter` impls → dependency-analysis caller
- Edge production (index → adjacency list) → `normalize-facts` (owns the index)
- `build_import_graph` and `ImportGraph` → `normalize-architecture` or `normalize-facts`

### Kill the duplication — RESOLVED 2026-07-01

`normalize-architecture` reimplemented `find_longest_chains`/`longest_path_from` with a
threshold that *looked* different (`>3` nodes vs. `>= MIN_CHAIN_NODE_COUNT` where the
constant is 4) but was in fact numerically identical (`> 3` ≡ `>= 4`) — a magic-number vs.
named-constant inconsistency, not a behavioral divergence. The only real difference was a
hard-coded limit of 5 in the architecture copy vs. the parameterized `limit` in the
canonical version. The duplicate had no callers anywhere in the workspace. Fixed by deleting
both functions from `normalize-architecture` and re-exporting the canonical versions from
`normalize-graph` (`pub use normalize_graph::{find_longest_chains, longest_path_from}`).
Pure internal dedup, no user-facing behavior change. The larger graph-crate split remains
deferred.

### Design so standalone extraction is mechanical

No normalize dependencies in the generic half from day one. Extraction to a standalone repo
later (if warranted by adoption) becomes a mechanical file-copy, not a refactor.

---

## Deferred — Not Mid-Migration

This is a deliberate project, not a quick cleanup. It requires:

- API design decision: trait-based (petgraph-style generic across representations with
  monomorphized perf) vs. plain generic functions over a neighbors-closure.
- Positioning relative to petgraph and the `pathfinding` crate — what is the honest claim
  for standalone value?
- Decision on whether to publish as its own repo or a workspace crate with `publish = true`.
- Naming (no LLM suggestions per CLAUDE.md; user decides).
- Documentation and examples if publishing standalone.

**Execution prerequisite:** the taxonomy inversion (B2 `graph` verb, B3 `architecture` verb)
must land first. B2 and B3 are BLOCKED on resolving this boundary — the `graph` and
`architecture` verbs can't cleanly land until it's clear which types stay in
`normalize-graph` and which move to the callers. The split defines what `normalize-graph`'s
`cli` feature gates, what `GraphService` exposes, and where the `OutputFormatter` impls live.

See `docs/artifacts/cli-taxonomy-2026-06-29/00-inversion-plan.md` B2–B3 for the verb-side
work that depends on this decision.

---

## Open Sub-Questions for the Focused Effort

1. **Trait vs. functions:** petgraph uses a `GraphBase` trait hierarchy so algorithms
   monomorphize. Plain generic functions over a `FnMut(node) -> impl Iterator<Item=node>`
   neighbors closure are simpler and zero-allocation but lose some ergonomic discoverability.
   Which is more honest for this crate's actual use case?

2. **Repo vs. workspace crate:** a separate repo signals "standalone tool, not internal
   detail." A workspace crate with `publish = true` is lower overhead. Decision depends on
   how credible the standalone-usefulness claim is after API design.

3. **Naming:** user decides; not a question for this record.

4. **Duplication dedup timing:** RESOLVED 2026-07-01 — done independently of the extraction.
   `normalize-architecture` now re-exports the canonical `find_longest_chains`/
   `longest_path_from` from `normalize-graph`; the local copies are deleted. The threshold
   "difference" was cosmetic (`> 3` ≡ `>= 4`). See "Kill the duplication" above.
