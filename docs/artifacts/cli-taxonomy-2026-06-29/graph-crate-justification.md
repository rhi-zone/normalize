# normalize-graph crate justification audit
_Generated 2026-07-01. Read-only investigation; no source changes._

---

## Q1: Does the workspace use petgraph or any other general graph library?

No. A grep of all `crates/*/Cargo.toml` for `petgraph`, `daggy`, and `graph` (excluding normalize-graph itself) returns no graph-library dependencies. The workspace carries zero external graph crate dependencies. All graph algorithms in `normalize-graph` are hand-rolled from scratch using only `std::collections::{HashMap, HashSet, VecDeque}`.

---

## Q2: Why were the graph algorithms hand-rolled?

There is **no stated rationale** in the source. The module-level doc comment says:

> "Operates on abstract graphs represented as adjacency lists (`HashMap<String, HashSet<String>>`). No filesystem, CLI, or normalize-specific types."

That describes the design but gives no reason for not using petgraph. There are no comments explaining why petgraph was rejected, no CHANGELOG entries about this choice, and no references to performance, licensing, or API ergonomics tradeoffs. The algorithms (Tarjan SCC, iterative bridge-finding, BFS, diamond detection, longest-chains with memoization, transitive edge detection) look written from scratch without any stated rationale for avoiding an existing lib.

---

## Q3: Are the algorithms generically parameterizable or String-entangled?

The algorithms operate on `HashMap<String, HashSet<String>>`. **Every public function signature is pinned to `String` keys.** They are not generic over `<N: Hash + Eq + Clone>` or similar — they use `.clone()`, `.to_string()`, and string operations throughout.

Representative example — `find_dependents`:

```rust
pub fn find_dependents(imports: &HashMap<String, HashSet<String>>, target: &str) -> Vec<String> {
    let rev = reverse_graph(imports);
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(target.to_string());
    visited.insert(target.to_string());
    while let Some(node) = queue.pop_front() {
        if let Some(parents) = rev.get(&node) {
            for parent in parents {
                if visited.insert(parent.clone()) {
                    queue.push_back(parent.clone());
                }
            }
        }
    }
    let mut result: Vec<String> = visited.into_iter().filter(|n| n != target).collect();
    result.sort();
    result
}
```

Tarjan SCC (`tarjan_sccs`) is the same: `HashMap<String, usize>` for `indices`/`lowlink`, `HashSet<String>` for `on_stack`, `Vec<String>` for the stack and SCC output. The bridge-finding algorithm converts to `Vec<String>` node lists and `HashMap<&str, usize>` index maps.

Making these generic would require replacing every `String` with `N: Hash + Eq + Clone + Ord`, every `.to_string()` with `.clone()`, every `&str`-keyed lookup with `&N`. The algorithms are not entangled with normalize _semantics_ (no module-name parsing, no path resolution, no DB calls) — but they are concretely entangled with `String` as the node type. Making them generic is mechanical but non-trivial refactoring.

---

## Q4: How many distinct workspace crates consume normalize-graph?

Exactly **2** crates declare `normalize-graph` as a dependency in their `Cargo.toml`:

1. `crates/normalize/Cargo.toml` — the main binary crate
2. `crates/normalize-architecture/Cargo.toml`

Usage breakdown:
- `normalize` (main crate): imports `BlastRadius`, `DependentEntry`, `analyze_graph_data`, `find_dependents`, `DependentsReport`, `GraphReport`, `GraphTarget` — uses the full analysis surface.
- `normalize-architecture`: re-exports `normalize_graph::ImportChain` and calls its own `find_longest_chains` which takes `&HashMap<String, HashSet<String>>` and returns `Vec<ImportChain>`.

This is **2 real workspace dependents**, which meets bar (a) in CLAUDE.md: "multiple actual dependents within the workspace."

---

## Q5: Generic-graph part vs normalize-specific part

**Generic-graph (could be petgraph equivalents):**
- `tarjan_sccs` / `find_sccs` — SCC detection (petgraph: `kosaraju_scc` / `tarjan_scc`)
- `find_bridges` — bridge finding (no petgraph built-in, but graph-algorithms crates have it)
- `weakly_connected_components` — WCC (petgraph: `connected_components` on undirected)
- `find_diamonds` — diamond pattern detection (not in petgraph, custom)
- `find_transitive_edges` / `count_transitive_edges` — transitive reduction detection (petgraph: `transitive_reduction` exists but is different)
- `find_longest_chains` / `longest_path_from` — longest path with memoization (petgraph: no direct equivalent for DAG longest path with cycle avoidance)
- `find_dead_nodes` — nodes with zero in-degree (petgraph: manual, trivial)
- `all_nodes`, `edge_count`, `reverse_graph` — graph utilities (petgraph: built-in)
- `find_dependents` — BFS on reversed graph (petgraph: `Bfs` on reversed graph)

**normalize-specific (cannot be replaced by petgraph):**
- `GraphTarget` enum — `Modules` / `Symbols` / `Types` — normalize domain concept
- `DependentEntry` struct — `has_tests: bool`, `fan_in: usize`, `depth: usize`, `file: String` — enriched with normalize-DB fields (test coverage, fan-in from index)
- `BlastRadius` struct — `untested_count`, `direct_count`, `transitive_count` — normalize-specific blast radius metrics
- `DependentsReport` — structured output mixing graph results with test coverage data
- `GraphReport` — full report struct with `OutputFormatter` impl (normalize-output dependency)
- `ImportChain` — used by `normalize-architecture` as its return type
- `OutputFormatter` implementations on all report structs (`format_text`, `format_pretty` with ANSI coloring)

The split is roughly: **~60% generic graph algorithms that could use petgraph** (though some like longest-path and diamond detection aren't in petgraph anyway) + **~40% normalize-specific report shaping, enrichment fields, and OutputFormatter impls** that have no petgraph equivalent.

---

## Bottom line

**Does normalize-graph clear the crate-existence bar?**

Barely — by bar (a) only: 2 distinct workspace dependents (`normalize` + `normalize-architecture`). No evidence it would be "clearly useful standalone" in the petgraph-ecosystem sense; it's not generic over node type, so no external Rust user would reach for it over petgraph.

**The actual question is: should the generic part be petgraph + specific part in-tree?**

Evidence against petgraph replacement:
- Several algorithms (diamond detection, longest-path with suffix dedup, normalize-style bridge reporting) have no petgraph equivalent and would need to be written regardless.
- petgraph uses its own `NodeIndex`/`EdgeIndex` typed graph, not `HashMap<String, HashSet<String>>`. Bridging between normalize's adjacency-list representation and petgraph's graph type adds non-trivial conversion overhead and API surface.
- The algorithms are correct, iterative (non-recursive, avoids stack overflow on large graphs), and well-documented. Replacing them buys no obvious correctness or performance win.
- Adding petgraph (a non-trivial dep with ~20K LOC) to avoid maintaining ~500 LOC of hand-rolled algorithms is a questionable trade on dep-weight grounds.

Evidence for collapse:
- The crate's public API is pinned to `String` nodes; it's not reusable outside normalize as-is.
- `normalize-architecture` only uses `ImportChain` — a type, not any algorithm. That type could live in `normalize-architecture` itself or be inlined.
- If `normalize-architecture`'s `ImportChain` use were moved in-tree, `normalize-graph` would have exactly 1 dependent (`normalize` main crate), failing bar (a).

**Recommendation (evidence-based):** The crate's existence is marginal. The strongest argument for keeping it separate is that `normalize-architecture` consumes it, but that usage is thin (one re-exported type). If `ImportChain` moved to `normalize-architecture`, the crate fails bar (a) and the algorithms + report structs should live in `crates/normalize/src/commands/analyze/graph.rs` per CLAUDE.md's "purely 'compute something and format it for this one command'" rule. The generic algorithms are not generic enough to be standalone, and petgraph is a poor fit for the adjacency-list API normalize uses.
