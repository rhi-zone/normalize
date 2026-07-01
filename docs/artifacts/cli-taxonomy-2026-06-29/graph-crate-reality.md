# normalize-graph: Reality Check

**Investigated:** 2026-06-30
**Files read:** `crates/normalize-graph/src/lib.rs`, `crates/normalize-architecture/src/lib.rs`, both `Cargo.toml` files, `crates/normalize-facts/Cargo.toml`

---

## Verdict: normalize-graph is NOT a general-purpose graph library

The crate's own module doc says:

> "Operates on abstract graphs represented as adjacency lists (`HashMap<String, HashSet<String>>`). No filesystem, CLI, or normalize-specific types."

That claim is false. The algorithms are implemented generically over `HashMap<String, HashSet<String>>`, but the types are tightly coupled to the import/call/type-dependency domain.

### Evidence: normalize-graph types are domain-specific

```rust
// GraphTarget is normalize-specific — it names the three dependency graph kinds
pub enum GraphTarget {
    Modules,   // files, edges are imports
    Symbols,   // functions, edges are calls
    Types,     // types, edges are type references
}

// ImportChain is named after imports, not abstract graph paths
pub struct ImportChain {
    pub modules: Vec<String>,  // called "modules" not "nodes"
    pub depth: usize,
}

// Scc fields say "modules", not "nodes"
pub struct Scc {
    pub modules: Vec<String>,
    pub internal_edges: usize,
}

// DependentEntry has a has_tests field — entirely project-specific concern
pub struct DependentEntry {
    pub file: String,
    pub depth: usize,
    pub has_tests: bool,   // test coverage is not a graph-theoretic concept
    pub fan_in: usize,
}

// BridgeEdge field docs say "The importing module"
pub struct BridgeEdge {
    pub from: String,  // "The importing module"
    pub to: String,    // "The imported module"
}
```

The report types — `GraphReport`, `DependentsReport`, `GraphStats` — have fields like
`dead_node_count` documented as "unreachable or potentially dead code" and comments
throughout use "import" and "module" as the conceptual frame, not generic graph terms.

`analyze_graph_data` takes a `GraphTarget` (import/call/type) and returns a
`GraphReport` — the top-level API is explicitly about the three normalize dependency
graph kinds, not about abstract graph analysis.

---

## The import-graph concern is scattered across TWO crates

### normalize-graph provides
- Algorithms: `tarjan_sccs`, `find_bridges`, `find_diamonds`, `find_transitive_edges`, `count_transitive_edges`, `find_longest_chains`, `find_dependents`, `find_dead_nodes`, `weakly_connected_components`, `all_nodes`, `edge_count`, `reverse_graph`
- Report types with formatting: `GraphReport`, `DependentsReport`, `GraphStats`, `ImportChain`, `Scc`, `Diamond`, `BridgeEdge`, `TransitiveEdge`, `DependentEntry`, `BlastRadius`
- Domain enum: `GraphTarget`

### normalize-architecture provides
- `ImportGraph` struct (the actual graph data model — constructed from DB):
  ```rust
  pub struct ImportGraph {
      pub imports_by_file: HashMap<String, HashSet<String>>,
      pub importers_by_file: HashMap<String, HashSet<String>>,
      pub raw_import_count: usize,
  }
  ```
- `build_import_graph(idx: &FileIndex) -> Result<ImportGraph, libsql::Error>` — queries the DB, resolves module names to files, builds the adjacency list
- Architectural analysis types: `Cycle`, `ModuleCoupling`, `HubModule`, `CrossImport`, `OrphanModule`, `LayerFlow`, `LayeringModuleResult`
- DUPLICATE `find_longest_chains` and `longest_path_from` — the same algorithm as in normalize-graph, independently reimplemented with slightly different thresholds (>3 nodes vs MIN_CHAIN_NODE_COUNT=4)
- `find_cycles` — cycle detection via iterative DFS (normalize-graph has `tarjan_sccs` for the same purpose)

### The duplication is concrete

normalize-architecture's `find_longest_chains` (line 496) and `longest_path_from` (line 541) are functionally identical to normalize-graph's versions. normalize-architecture imports `ImportChain` from normalize-graph (`pub use normalize_graph::ImportChain;`) but then reimplements the function that produces them. This is an outright code smell.

---

## Dependency directions

```
normalize-architecture
  -> normalize-graph   (uses ImportChain type and graph algorithms)
  -> normalize-facts   (FileIndex for DB access)
  -> normalize-languages (is_programming_language filter)

normalize-facts
  (does NOT depend on normalize-graph)

normalize-graph
  -> normalize-output  (OutputFormatter trait)
  (no dependency on normalize-facts or normalize-architecture)
```

No cycle exists in the crate dependency graph itself. The prior "cycle risk" report was
either wrong or referring to something else — normalize-architecture depends on
normalize-graph (not vice versa), and normalize-facts does not depend on normalize-graph.

---

## Summary

The "pure primitives / composition" framing is partially true and partially false:

- TRUE: The algorithms in normalize-graph are implemented over a generic
  `HashMap<String, HashSet<String>>` adjacency list. They are not hardcoded to any
  specific data source and will work on any string-keyed directed graph.

- FALSE: The types, enum variants, field names, and documentation all speak the
  import/call/type-dependency language. There is no `Graph<N, E>`. An LLM or a user
  who reads "pure graph primitives" will expect `petgraph`-style generics; what they
  get is an import-graph analysis crate whose adjacency list happens to use strings.

- SCATTERED: `ImportGraph` construction (DB → adjacency list) lives in
  normalize-architecture, not in normalize-graph. normalize-architecture then
  duplicates two non-trivial functions (`find_longest_chains`, `longest_path_from`)
  from normalize-graph.

The honest description: normalize-graph is a dependency-graph analysis library (for
the modules/symbols/types dependency graphs that normalize indexes), not a general
graph library. The import-graph concern is split between the two crates with
duplication. Consolidation path: remove the duplicate `find_longest_chains` /
`longest_path_from` from normalize-architecture and call normalize-graph's versions,
or move `build_import_graph` + the `ImportGraph` type into normalize-graph.
