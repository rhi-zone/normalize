# normalize-graph/src

Single-file source for the `normalize-graph` crate.

`lib.rs` contains only pure graph algorithms plus their plain result types and a `#[cfg(test)]` characterization suite — no presentation, no normalize types. Input is always `&HashMap<String, HashSet<String>>` (adjacency list). Algorithms use iterative DFS with explicit stacks to avoid stack overflow on large graphs. `find_longest_chains` uses memoized DFS with suffix-dominance deduplication (an APPROXIMATE, cycle-tolerant longest-simple-path heuristic — its through-cycle output is order-dependent by design). Report assembly and `OutputFormatter` rendering moved to `crates/normalize/src/commands/analyze/graph.rs`.

The test suite mixes characterization tests (pinning current behavior for algorithms whose output is heuristic/approximate, e.g. `find_longest_chains`) with correct-output tests that assert real correctness for `tarjan_sccs`/`find_sccs` (2-cycle, 3-cycle, self-loop, nested/multiple SCCs, pure DAG, disconnected cycles, determinism) and `find_bridges` (bridge vs cycle edge). All graphs are small and deterministic; assertions sort where HashMap ordering would otherwise leak.
