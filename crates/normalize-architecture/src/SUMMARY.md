# normalize-architecture/src

Single-file source for the `normalize-architecture` crate.

`lib.rs` contains all types and algorithms: `build_import_graph` (async, queries the index), `compute_coupling_and_hubs`, `detect_cross_imports`, `find_orphan_modules`, `find_symbol_hotspots`, `find_cycles` (DFS), `extract_layer`, `compute_layer_flows`, `compute_depth`, `compute_downstream`, and `compute_layering_compliance`. `ImportChain` and `find_longest_chains` are re-exported from `normalize-graph`.
