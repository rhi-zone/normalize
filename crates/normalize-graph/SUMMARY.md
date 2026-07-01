# normalize-graph

Pure graph algorithms for dependency analysis, operating on abstract adjacency lists (`&HashMap<String, HashSet<String>>`) with no filesystem, CLI, or normalize-specific types. Presentation (report assembly, `OutputFormatter`, `GraphTarget`) lives in the consumer — `crates/normalize/src/commands/analyze/graph.rs`.

Plain result/data types (serde + schemars derives only): `Scc`, `Diamond`, `BridgeEdge`, `ImportChain`, `TransitiveEdge`, `DependentEntry`, `BlastRadius`. Functions: `tarjan_sccs`, `find_sccs`, `find_diamonds`, `find_bridges`, `find_longest_chains` (+ `longest_path_from`), `find_transitive_edges`, `count_transitive_edges`, `weakly_connected_components`, `find_dead_nodes`, `find_dependents`, `reverse_graph`, `all_nodes`, `edge_count`. Single-module crate (all logic in `lib.rs`) with a `#[cfg(test)]` characterization suite. Published as a standalone crate on crates.io; usable independently of normalize.

Known bug (pinned by the characterization tests, not yet fixed): the iterative `tarjan_sccs` pushes its `Frame::Resume` sentinel after the neighbor frames, so the SCC-root check runs before children update `lowlink` — it returns all singletons even for real cycles, making `find_sccs` always empty. See `TODO.md`.
