# normalize-graph/src

Single-file source for the `normalize-graph` crate.

`lib.rs` contains only pure graph algorithms plus their plain result types and a `#[cfg(test)]` characterization suite — no presentation, no normalize types. Input is always `&HashMap<String, HashSet<String>>` (adjacency list). Algorithms use iterative DFS with explicit stacks to avoid stack overflow on large graphs. `find_longest_chains` uses memoized DFS with suffix-dominance deduplication (an APPROXIMATE, cycle-tolerant longest-simple-path heuristic — its through-cycle output is order-dependent by design). Report assembly and `OutputFormatter` rendering moved to `crates/normalize/src/commands/analyze/graph.rs`.

The characterization tests pin current behavior on small deterministic graphs, including the known `tarjan_sccs` singleton bug (see the crate `SUMMARY.md`) — they lock behavior, they do not assert correctness.
