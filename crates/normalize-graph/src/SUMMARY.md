# normalize-graph/src

Single-file source for the `normalize-graph` crate.

`lib.rs` contains all graph algorithms and the `GraphReport` `OutputFormatter` implementation. Input is always `&HashMap<String, HashSet<String>>` (adjacency list). Algorithms use iterative DFS with explicit stacks to avoid stack overflow on large graphs. `find_longest_chains` uses memoized DFS with suffix-dominance deduplication. `format_text` and `format_pretty` on `GraphReport` render all sections (SCCs, diamonds, bridges, chains, transitive edges) with optional ANSI colors.
