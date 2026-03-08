# normalize-graph

Pure graph algorithms for dependency analysis, operating on abstract adjacency lists with no normalize-specific types.

Key types: `GraphTarget` (Modules/Symbols/Types), `GraphReport`, `GraphStats`, `Scc`, `Diamond`, `BridgeEdge`, `ImportChain`, `TransitiveEdge`, `DependentsReport`. Key functions: `analyze_graph_data` (top-level entry point), `tarjan_sccs` (iterative Tarjan's SCC), `find_sccs`, `find_diamonds`, `find_bridges` (iterative Tarjan's bridge algorithm), `find_transitive_edges`, `find_longest_chains`, `weakly_connected_components`, `find_dead_nodes` (nodes with no inbound edges), `find_dependents` (reverse BFS), `reverse_graph`. `GraphReport` and `DependentsReport` implement `OutputFormatter` with pretty/text rendering.
