# Decision Record: normalize-graph Crate Architecture

**Decided:** 2026-07-01
**Status:** DECIDED IN PRINCIPLE — DEFERRED (not to execute mid-taxonomy migration)
**Evidence artifacts:** `graph-crate-reality.md`, `graph-crate-justification.md`

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

### Kill the duplication

`normalize-architecture` reimplements `find_longest_chains`/`longest_path_from` with
slightly different thresholds (>3 nodes vs. MIN_CHAIN_NODE_COUNT=4). This is an outright
bug. Consolidate to one implementation when executing this split.

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

4. **Duplication dedup timing:** the `find_longest_chains` duplication in
   `normalize-architecture` is a bug independent of this extraction. It can be fixed before
   the full split by having `normalize-architecture` call `normalize-graph`'s version — but
   that's a short-term patch; the extraction resolves it permanently.
