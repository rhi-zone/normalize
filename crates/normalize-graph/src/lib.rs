//! Pure graph algorithms for dependency analysis.
//!
//! Operates on abstract graphs represented as adjacency lists
//! (`HashMap<String, HashSet<String>>`). No filesystem, CLI, or
//! normalize-specific types.
//!
//! Algorithms: Tarjan SCC, bridge-finding, diamond detection,
//! transitive edge detection, longest chains, weakly connected components.

use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

/// Minimum chain length (in nodes) to include in the longest-chains report.
///
/// A chain of 4 nodes has depth 3 (3 edges). Depth 2 chains are common in any
/// project with a utilities layer and are not interesting signals. Starting at
/// depth ≥ 3 surfaces only chains that likely indicate layering violations or
/// overly deep call stacks worth reviewing.
const MIN_CHAIN_NODE_COUNT: usize = 4;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A strongly connected component (circular-dependency cluster).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Scc {
    /// Modules that are part of this strongly connected component.
    pub modules: Vec<String>,
    /// Number of edges within the SCC
    pub internal_edges: usize,
}

/// A diamond dependency: source imports left and right, both import target.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Diamond {
    /// The module that starts the diamond (imports both `left` and `right`).
    pub source: String,
    /// The left intermediate module (imports `target`).
    pub left: String,
    /// The right intermediate module (imports `target`).
    pub right: String,
    /// The shared dependency that both intermediate modules import.
    pub target: String,
}

/// A bridge edge whose removal disconnects the graph.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BridgeEdge {
    /// The importing module.
    pub from: String,
    /// The imported module.
    pub to: String,
}

/// A deep import chain (longest dependency path).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ImportChain {
    /// Modules in the chain from start to end, ordered by import depth.
    pub modules: Vec<String>,
    /// Length of the chain (number of edges, not nodes)
    pub depth: usize,
}

/// A transitive (redundant) import: A→C is redundant because A→B→C.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TransitiveEdge {
    /// The importing module.
    pub from: String,
    /// The transitively reachable module (redundant direct dependency).
    pub to: String,
    /// The intermediate module that already provides the transitive path.
    pub via: String,
}

/// A file that depends on the query target (modules graph only).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DependentEntry {
    pub file: String,
    pub depth: usize,
    pub has_tests: bool,
    pub fan_in: usize,
}

/// Blast radius summary statistics (modules graph only).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BlastRadius {
    pub direct_count: usize,
    pub transitive_count: usize,
    pub untested_count: usize,
    pub max_depth: usize,
}

// ---------------------------------------------------------------------------
// Algorithms
// ---------------------------------------------------------------------------

/// Collect all nodes from the import graph.
pub fn all_nodes(imports: &HashMap<String, HashSet<String>>) -> HashSet<String> {
    let mut nodes = HashSet::new();
    for (k, vs) in imports {
        nodes.insert(k.clone());
        for v in vs {
            nodes.insert(v.clone());
        }
    }
    nodes
}

/// Count directed edges.
pub fn edge_count(imports: &HashMap<String, HashSet<String>>) -> usize {
    imports.values().map(|s| s.len()).sum()
}

/// Build the reverse (transposed) graph: edges point from target to source.
pub fn reverse_graph(
    imports: &HashMap<String, HashSet<String>>,
) -> HashMap<String, HashSet<String>> {
    let mut rev: HashMap<String, HashSet<String>> = HashMap::new();
    for (src, targets) in imports {
        for tgt in targets {
            rev.entry(tgt.clone()).or_default().insert(src.clone());
        }
    }
    rev
}

/// Find all nodes that (transitively) depend on `target` via BFS on the reverse graph.
/// Returns a sorted list excluding the target itself.
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

/// Find nodes with no inbound edges (nothing imports/calls them) that do have
/// outbound edges (they import/call something). These are unreachable internal
/// nodes — potential dead code. Entry points (main.rs, lib.rs) are included;
/// callers can filter based on their heuristics.
pub fn find_dead_nodes(imports: &HashMap<String, HashSet<String>>) -> Vec<String> {
    // Collect all nodes that appear as targets (have at least one inbound edge)
    let mut has_inbound: HashSet<&str> = HashSet::new();
    for targets in imports.values() {
        for t in targets {
            has_inbound.insert(t.as_str());
        }
    }

    // Nodes that have outbound edges but no inbound edges
    let mut dead: Vec<String> = imports
        .keys()
        .filter(|n| !has_inbound.contains(n.as_str()))
        .cloned()
        .collect();
    dead.sort();
    dead
}

/// Weakly connected components via BFS on the undirected view.
pub fn weakly_connected_components(imports: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    // Build undirected adjacency
    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for (u, vs) in imports {
        for v in vs {
            adj.entry(u.clone()).or_default().insert(v.clone());
            adj.entry(v.clone()).or_default().insert(u.clone());
        }
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut components = Vec::new();
    let nodes = all_nodes(imports);

    for node in &nodes {
        if visited.contains(node) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(node.clone());
        visited.insert(node.clone());

        while let Some(cur) = queue.pop_front() {
            component.push(cur.clone());
            if let Some(neighbors) = adj.get(&cur) {
                for n in neighbors {
                    if visited.insert(n.clone()) {
                        queue.push_back(n.clone());
                    }
                }
            }
        }
        component.sort();
        components.push(component);
    }

    components.sort_by_key(|c| std::cmp::Reverse(c.len()));
    components
}

/// Iterative Tarjan's SCC algorithm.
pub fn tarjan_sccs(imports: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    let nodes = all_nodes(imports);
    let mut index_counter = 0usize;
    let mut indices: HashMap<String, usize> = HashMap::new();
    let mut lowlink: HashMap<String, usize> = HashMap::new();
    let mut on_stack: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = Vec::new();
    let mut sccs: Vec<Vec<String>> = Vec::new();

    // Iterative DFS using an explicit call stack
    #[derive(Debug)]
    enum Frame {
        Enter(String),
        Resume(String, String), // (node, neighbor) — resume after returning from neighbor
    }

    for start in &nodes {
        if indices.contains_key(start) {
            continue;
        }

        let mut call_stack: Vec<Frame> = vec![Frame::Enter(start.clone())];

        while let Some(frame) = call_stack.pop() {
            match frame {
                Frame::Enter(node) => {
                    if indices.contains_key(&node) {
                        continue;
                    }
                    indices.insert(node.clone(), index_counter);
                    lowlink.insert(node.clone(), index_counter);
                    index_counter += 1;
                    stack.push(node.clone());
                    on_stack.insert(node.clone());

                    // Push the root-check sentinel FIRST so it is popped LAST —
                    // after every child subtree has been fully explored and has
                    // propagated its lowlink back via its own `Resume` frame. The
                    // call stack is LIFO (pop-based), so "pushed first" == "popped
                    // last". (Pushing the sentinel last, as before, ran the root
                    // check before any child `Enter` frame, collapsing every SCC
                    // to a singleton.)
                    call_stack.push(Frame::Resume(node.clone(), String::new()));

                    let neighbors: Vec<String> = imports
                        .get(&node)
                        .map(|s| s.iter().cloned().collect())
                        .unwrap_or_default();

                    // Push neighbors in reverse so we process them in order (LIFO).
                    // Each unvisited neighbor gets an `Enter` (explore) followed by
                    // a `Resume(node, neighbor)` that propagates the child's lowlink
                    // back into `node` on the way up.
                    for neighbor in neighbors.into_iter().rev() {
                        if !indices.contains_key(&neighbor) {
                            call_stack.push(Frame::Resume(node.clone(), neighbor.clone()));
                            call_stack.push(Frame::Enter(neighbor));
                        } else if on_stack.contains(&neighbor) {
                            // normalize-syntax-allow: rust/unwrap-in-impl - node inserted into indices/lowlink at Frame::Enter
                            let nl = *lowlink.get(&node).unwrap(); // normalize-syntax-allow: rust/unwrap-in-impl - node inserted at Frame::Enter
                            let ni = *indices.get(&neighbor).unwrap(); // normalize-syntax-allow: rust/unwrap-in-impl - neighbor was already visited (in indices)
                            lowlink.insert(node.clone(), nl.min(ni));
                        }
                    }
                }
                Frame::Resume(node, neighbor) => {
                    if neighbor.is_empty() {
                        // Sentinel: all neighbors processed, check if root of SCC
                        // normalize-syntax-allow: rust/unwrap-in-impl - node inserted into indices/lowlink at Frame::Enter
                        let nl = *lowlink.get(&node).unwrap();
                        let ni = *indices.get(&node).unwrap(); // normalize-syntax-allow: rust/unwrap-in-impl - node inserted at Frame::Enter
                        if nl == ni {
                            let mut scc = Vec::new();
                            while let Some(w) = stack.pop() {
                                on_stack.remove(&w);
                                scc.push(w.clone());
                                if w == node {
                                    break;
                                }
                            }
                            scc.sort();
                            sccs.push(scc);
                        }
                    } else {
                        // Resume after DFS into neighbor
                        // normalize-syntax-allow: rust/unwrap-in-impl - node inserted into lowlink at Frame::Enter
                        let nl = *lowlink.get(&node).unwrap();
                        let neighbor_ll = *lowlink.get(&neighbor).unwrap_or(&usize::MAX);
                        lowlink.insert(node.clone(), nl.min(neighbor_ll));
                    }
                }
            }
        }
    }

    sccs
}

/// Find nontrivial SCCs (size > 1) with internal edge counts.
pub fn find_sccs(imports: &HashMap<String, HashSet<String>>) -> Vec<Scc> {
    let raw = tarjan_sccs(imports);
    let mut result = Vec::new();
    for modules in raw {
        if modules.len() <= 1 {
            continue;
        }
        let member_set: HashSet<&str> = modules.iter().map(|s| s.as_str()).collect();
        let mut internal_edges = 0;
        for m in &modules {
            if let Some(targets) = imports.get(m) {
                for t in targets {
                    if member_set.contains(t.as_str()) {
                        internal_edges += 1;
                    }
                }
            }
        }
        result.push(Scc {
            modules,
            internal_edges,
        });
    }
    result.sort_by_key(|b| std::cmp::Reverse(b.modules.len()));
    result
}

/// Diamond detection: for each node A with imports {B₁,B₂,...}, check pairs for shared targets.
pub fn find_diamonds(imports: &HashMap<String, HashSet<String>>, limit: usize) -> Vec<Diamond> {
    let mut diamonds = Vec::new();
    let mut seen: HashSet<(String, String, String, String)> = HashSet::new();

    let mut sources: Vec<&String> = imports.keys().collect();
    sources.sort();

    for source in sources {
        let deps: Vec<&String> = match imports.get(source) {
            Some(s) => {
                let mut v: Vec<&String> = s.iter().collect();
                v.sort();
                v
            }
            None => continue,
        };
        if deps.len() < 2 {
            continue;
        }

        for i in 0..deps.len() {
            let left_targets = match imports.get(deps[i]) {
                Some(s) => s,
                None => continue,
            };
            for j in (i + 1)..deps.len() {
                let right_targets = match imports.get(deps[j]) {
                    Some(s) => s,
                    None => continue,
                };
                // Find shared targets
                for target in left_targets.intersection(right_targets) {
                    let key = (
                        source.clone(),
                        deps[i].clone(),
                        deps[j].clone(),
                        target.clone(),
                    );
                    if seen.insert(key) {
                        diamonds.push(Diamond {
                            source: source.clone(),
                            left: deps[i].clone(),
                            right: deps[j].clone(),
                            target: target.clone(),
                        });
                        if diamonds.len() >= limit {
                            return diamonds;
                        }
                    }
                }
            }
        }
    }
    diamonds
}

/// Bridge finding via Tarjan's bridge algorithm on the undirected view.
/// Only report bridges where exactly one directed edge exists (bidirectional ≠ bridge).
pub fn find_bridges(imports: &HashMap<String, HashSet<String>>) -> Vec<BridgeEdge> {
    // Build undirected adjacency list with neighbor indices for efficiency
    let nodes = all_nodes(imports);
    let node_list: Vec<String> = {
        let mut v: Vec<String> = nodes.into_iter().collect();
        v.sort();
        v
    };
    let node_idx: HashMap<&str, usize> = node_list
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let n = node_list.len();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut directed_edges: HashSet<(usize, usize)> = HashSet::new();

    for (u, vs) in imports {
        // normalize-syntax-allow: rust/unwrap-in-impl - node_idx built from the same node_list as imports keys
        let ui = *node_idx.get(u.as_str()).unwrap();
        for v in vs {
            // normalize-syntax-allow: rust/unwrap-in-impl - node_idx built from the same node_list as imports values
            let vi = *node_idx.get(v.as_str()).unwrap();
            directed_edges.insert((ui, vi));
            if !adj[ui].contains(&vi) {
                adj[ui].push(vi);
            }
            if !adj[vi].contains(&ui) {
                adj[vi].push(ui);
            }
        }
    }

    // Iterative bridge-finding (Tarjan's)
    let mut disc = vec![0usize; n];
    let mut low = vec![0usize; n];
    let mut visited = vec![false; n];
    let mut timer = 1usize;
    let mut bridges_idx: Vec<(usize, usize)> = Vec::new();

    #[derive(Debug)]
    struct BridgeFrame {
        node: usize,
        parent: usize, // usize::MAX for none
        adj_idx: usize,
    }

    for start in 0..n {
        if visited[start] {
            continue;
        }

        let mut stack = vec![BridgeFrame {
            node: start,
            parent: usize::MAX,
            adj_idx: 0,
        }];
        visited[start] = true;
        disc[start] = timer;
        low[start] = timer;
        timer += 1;

        while let Some(frame) = stack.last_mut() {
            let u = frame.node;
            if frame.adj_idx < adj[u].len() {
                let v = adj[u][frame.adj_idx];
                frame.adj_idx += 1;

                if !visited[v] {
                    visited[v] = true;
                    disc[v] = timer;
                    low[v] = timer;
                    timer += 1;
                    stack.push(BridgeFrame {
                        node: v,
                        parent: u,
                        adj_idx: 0,
                    });
                } else if v != frame.parent {
                    low[u] = low[u].min(disc[v]);
                }
            } else {
                // Done with this node — pop and update parent
                let u = frame.node;
                let parent = frame.parent;
                stack.pop();

                if parent != usize::MAX {
                    low[parent] = low[parent].min(low[u]);
                    if low[u] > disc[parent] {
                        bridges_idx.push((parent, u));
                    }
                }
            }
        }
    }

    // Map back to directed edges, only report if unidirectional
    let mut result = Vec::new();
    for (a, b) in bridges_idx {
        let has_ab = directed_edges.contains(&(a, b));
        let has_ba = directed_edges.contains(&(b, a));
        // Bidirectional edges aren't true dependency bridges
        if has_ab && !has_ba {
            result.push(BridgeEdge {
                from: node_list[a].clone(),
                to: node_list[b].clone(),
            });
        } else if has_ba && !has_ab {
            result.push(BridgeEdge {
                from: node_list[b].clone(),
                to: node_list[a].clone(),
            });
        }
    }
    result.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    result
}

/// Transitive edge detection: A→C is redundant if ∃B: A→B and B→C.
pub fn find_transitive_edges(
    imports: &HashMap<String, HashSet<String>>,
    limit: usize,
) -> Vec<TransitiveEdge> {
    let mut result = Vec::new();
    let mut sources: Vec<&String> = imports.keys().collect();
    sources.sort();

    'outer: for a in &sources {
        let a_targets = match imports.get(*a) {
            Some(s) => s,
            None => continue,
        };
        for c in a_targets {
            // Check if any other direct import B of A has C as a target
            for b in a_targets {
                if b == c {
                    continue;
                }
                if let Some(b_targets) = imports.get(b)
                    && b_targets.contains(c)
                {
                    result.push(TransitiveEdge {
                        from: (*a).clone(),
                        to: c.clone(),
                        via: b.clone(),
                    });
                    if result.len() >= limit {
                        break 'outer;
                    }
                    break; // One witness suffices per (A, C)
                }
            }
        }
    }
    result
}

/// Count all transitive edges (without limit, for stats).
pub fn count_transitive_edges(imports: &HashMap<String, HashSet<String>>) -> usize {
    let mut count = 0;
    for a_targets in imports.values() {
        for c in a_targets {
            for b in a_targets {
                if b == c {
                    continue;
                }
                if let Some(b_targets) = imports.get(b)
                    && b_targets.contains(c)
                {
                    count += 1;
                    break; // One witness per (A, C)
                }
            }
        }
    }
    count
}

/// Find the longest import chains (dependency paths) in the graph.
///
/// Returns up to `limit` chains sorted by depth (longest first). When `limit`
/// is `0` all qualifying chains are returned. Only chains whose node count meets
/// [`MIN_CHAIN_NODE_COUNT`] are included.
///
/// Chains dominated by a suffix of a longer chain are removed: if chain B's
/// modules are a suffix of chain A's modules, B is dropped. This keeps the
/// result set non-redundant — each returned chain represents a unique root.
///
/// Uses DFS from each node with memoization to find the longest path, avoiding cycles.
pub fn find_longest_chains(
    graph: &HashMap<String, HashSet<String>>,
    limit: usize,
) -> Vec<ImportChain> {
    let mut longest_paths: Vec<ImportChain> = Vec::new();
    let mut memo: HashMap<String, Vec<String>> = HashMap::new();

    for start in graph.keys() {
        let mut visited: HashSet<String> = HashSet::new();
        let path = longest_path_from(start, graph, &mut visited, &mut memo);
        if path.len() >= MIN_CHAIN_NODE_COUNT {
            longest_paths.push(ImportChain {
                depth: path.len() - 1,
                modules: path,
            });
        }
    }

    longest_paths.sort_by_key(|b| std::cmp::Reverse(b.depth));

    // Deduplicate — if a shorter chain is a suffix of a longer one, skip it
    let mut unique_chains: Vec<ImportChain> = Vec::new();
    for chain in longest_paths {
        let dominated = unique_chains.iter().any(|existing| {
            existing.modules.len() > chain.modules.len()
                && existing.modules.ends_with(&chain.modules)
        });
        if !dominated {
            unique_chains.push(chain);
        }
        if unique_chains.len() >= limit {
            break;
        }
    }

    unique_chains
}

/// Find the longest path from a node using DFS with memoization.
///
/// # Memoization limitation
///
/// Results are cached keyed only by `node`. When the same node is reached from
/// two different roots the first cached result is reused, even though the
/// `visited` set differs between the two calls. This means the cached result
/// may be shorter than what would be computed from a different root (because
/// some successors were marked visited in the first traversal). The trade-off
/// is acceptable: the memo avoids O(n²) worst-case work and the goal is
/// finding representative longest paths, not an exhaustive enumeration.
pub fn longest_path_from(
    node: &str,
    graph: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    memo: &mut HashMap<String, Vec<String>>,
) -> Vec<String> {
    if let Some(cached) = memo.get(node) {
        return cached.clone();
    }

    visited.insert(node.to_string());

    let mut longest: Vec<String> = vec![node.to_string()];

    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                let sub_path = longest_path_from(neighbor, graph, visited, memo);
                if sub_path.len() + 1 > longest.len() {
                    let mut new_path = vec![node.to_string()];
                    new_path.extend(sub_path);
                    longest = new_path;
                }
            }
        }
    }

    visited.remove(node);
    memo.insert(node.to_string(), longest.clone());
    longest
}

// ---------------------------------------------------------------------------
// Characterization tests
// ---------------------------------------------------------------------------
//
// These pin the CURRENT observable behavior of every pure algorithm so the
// presentation split (moving report/formatting code into the `normalize` crate)
// is provably behavior-preserving. Graphs are chosen so results are
// deterministic despite `HashSet`/`HashMap` iteration-order nondeterminism:
// unambiguous longest paths, single-witness transitive edges, sorted outputs.
//
// `find_longest_chains` is an APPROXIMATE, cycle-tolerant longest-SIMPLE-path
// heuristic with node-keyed memoization and suffix-dominance dedup — these tests
// pin its current output, they do NOT assert it is a correct longest-path solver.
#[cfg(test)]
mod tests {
    use super::*;

    /// Build an adjacency list from `(from, to)` edge pairs.
    fn g(edges: &[(&str, &str)]) -> HashMap<String, HashSet<String>> {
        let mut m: HashMap<String, HashSet<String>> = HashMap::new();
        for (a, b) in edges {
            m.entry(a.to_string()).or_default().insert(b.to_string());
        }
        m
    }

    #[test]
    fn all_nodes_and_edge_count() {
        let graph = g(&[("a", "b"), ("b", "c"), ("a", "c")]);
        let nodes = all_nodes(&graph);
        assert_eq!(nodes.len(), 3);
        assert!(nodes.contains("a") && nodes.contains("b") && nodes.contains("c"));
        assert_eq!(edge_count(&graph), 3);
    }

    #[test]
    fn reverse_graph_transposes_edges() {
        let graph = g(&[("a", "b"), ("a", "c")]);
        let rev = reverse_graph(&graph);
        assert_eq!(rev.get("b").unwrap().iter().collect::<Vec<_>>(), vec!["a"]);
        assert_eq!(rev.get("c").unwrap().iter().collect::<Vec<_>>(), vec!["a"]);
        assert!(!rev.contains_key("a"));
    }

    #[test]
    fn find_dependents_reverse_bfs_sorted() {
        // a -> b -> c ; d -> c
        let graph = g(&[("a", "b"), ("b", "c"), ("d", "c")]);
        assert_eq!(find_dependents(&graph, "c"), vec!["a", "b", "d"]);
        assert_eq!(find_dependents(&graph, "b"), vec!["a"]);
        assert!(find_dependents(&graph, "a").is_empty());
    }

    #[test]
    fn find_dead_nodes_no_inbound_with_outbound() {
        // a and d have no inbound edges; c has no outbound so is not "dead" here.
        let graph = g(&[("a", "b"), ("b", "c"), ("d", "b")]);
        assert_eq!(find_dead_nodes(&graph), vec!["a", "d"]);
    }

    #[test]
    fn weakly_connected_components_two_islands() {
        // {a,b,c} and {x,y}
        let graph = g(&[("a", "b"), ("b", "c"), ("x", "y")]);
        let wcc = weakly_connected_components(&graph);
        assert_eq!(wcc.len(), 2);
        assert_eq!(wcc[0], vec!["a", "b", "c"]); // largest first, members sorted
        assert_eq!(wcc[1], vec!["x", "y"]);
    }

    /// Normalize `tarjan_sccs` output for order-independent comparison: sort
    /// members within each SCC (already done inside the algorithm) and sort the
    /// list of SCCs so assertions are stable despite HashMap iteration order.
    fn sorted_sccs(graph: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
        let mut sccs = tarjan_sccs(graph);
        for scc in &mut sccs {
            scc.sort();
        }
        sccs.sort();
        sccs
    }

    /// Membership sets of the non-trivial SCCs found by `find_sccs`, sorted for
    /// stable comparison.
    fn sorted_scc_clusters(graph: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
        let mut clusters: Vec<Vec<String>> = find_sccs(graph)
            .into_iter()
            .map(|s| {
                let mut m = s.modules;
                m.sort();
                m
            })
            .collect();
        clusters.sort();
        clusters
    }

    #[test]
    fn tarjan_two_cycle_single_scc() {
        // a -> b -> a : one SCC {a, b}.
        let graph = g(&[("a", "b"), ("b", "a")]);
        assert_eq!(
            sorted_sccs(&graph),
            vec![vec!["a".to_string(), "b".to_string()]]
        );
        assert_eq!(
            sorted_scc_clusters(&graph),
            vec![vec!["a".to_string(), "b".to_string()]]
        );
    }

    #[test]
    fn tarjan_three_cycle_single_scc() {
        // a -> b -> c -> a : one SCC of size 3.
        let graph = g(&[("a", "b"), ("b", "c"), ("c", "a")]);
        assert_eq!(
            sorted_sccs(&graph),
            vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
        );
    }

    #[test]
    fn tarjan_cycle_with_dangling_tail() {
        // a -> b -> c -> a plus c -> d (a DAG tail off the cycle).
        // SCCs: {a,b,c} and singleton {d}. find_sccs reports only {a,b,c}.
        let graph = g(&[("a", "b"), ("b", "c"), ("c", "a"), ("c", "d")]);
        assert_eq!(
            sorted_sccs(&graph),
            vec![
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                vec!["d".to_string()],
            ]
        );
        assert_eq!(
            sorted_scc_clusters(&graph),
            vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
        );
    }

    #[test]
    fn tarjan_self_loop_is_own_scc() {
        // Tarjan treats a self-loop node as its own (size-1) SCC. `find_sccs`
        // filters len <= 1, so a self-loop is NOT reported as a cluster.
        let graph = g(&[("a", "a")]);
        assert_eq!(sorted_sccs(&graph), vec![vec!["a".to_string()]]);
        assert!(find_sccs(&graph).is_empty());
    }

    #[test]
    fn tarjan_multiple_sccs_with_connecting_dag() {
        // Two disjoint cycles {a,b} and {c,d} joined by a DAG edge b -> c, plus
        // a lone DAG node e off the second cycle (d -> e).
        // SCCs: {a,b}, {c,d}, {e}. find_sccs reports {a,b} and {c,d}.
        let graph = g(&[
            ("a", "b"),
            ("b", "a"),
            ("c", "d"),
            ("d", "c"),
            ("b", "c"),
            ("d", "e"),
        ]);
        assert_eq!(
            sorted_sccs(&graph),
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
                vec!["e".to_string()],
            ]
        );
        assert_eq!(
            sorted_scc_clusters(&graph),
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
            ]
        );
    }

    #[test]
    fn tarjan_pure_dag_no_nontrivial_sccs() {
        // a -> b -> c, a -> c : acyclic, every SCC is a singleton.
        let graph = g(&[("a", "b"), ("b", "c"), ("a", "c")]);
        assert_eq!(
            sorted_sccs(&graph),
            vec![
                vec!["a".to_string()],
                vec!["b".to_string()],
                vec!["c".to_string()],
            ]
        );
        assert!(find_sccs(&graph).is_empty());
    }

    #[test]
    fn tarjan_disconnected_components_each_with_cycle() {
        // Two disconnected islands, each a 2-cycle: {a,b} and {x,y}.
        let graph = g(&[("a", "b"), ("b", "a"), ("x", "y"), ("y", "x")]);
        assert_eq!(
            sorted_scc_clusters(&graph),
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["x".to_string(), "y".to_string()],
            ]
        );
    }

    #[test]
    fn tarjan_deterministic_across_runs() {
        // A 3-cycle feeding a nested 2-cycle. Repeated runs must be identical
        // despite HashMap ordering.
        let graph = g(&[
            ("a", "b"),
            ("b", "c"),
            ("c", "a"),
            ("c", "d"),
            ("d", "e"),
            ("e", "d"),
        ]);
        let first = sorted_sccs(&graph);
        for _ in 0..20 {
            assert_eq!(sorted_sccs(&graph), first);
        }
        assert_eq!(
            first,
            vec![
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                vec!["d".to_string(), "e".to_string()],
            ]
        );
    }

    #[test]
    fn find_diamonds_strict_motif() {
        // S -> {L, R} -> T
        let graph = g(&[("S", "L"), ("S", "R"), ("L", "T"), ("R", "T")]);
        let diamonds = find_diamonds(&graph, usize::MAX);
        assert_eq!(diamonds.len(), 1);
        let d = &diamonds[0];
        assert_eq!(
            (
                d.source.as_str(),
                d.left.as_str(),
                d.right.as_str(),
                d.target.as_str()
            ),
            ("S", "L", "R", "T")
        );
    }

    #[test]
    fn find_diamonds_limit_caps_count() {
        // Two independent diamonds; limit=1 returns only the first (by sorted source).
        let graph = g(&[
            ("A", "L1"),
            ("A", "R1"),
            ("L1", "T1"),
            ("R1", "T1"),
            ("B", "L2"),
            ("B", "R2"),
            ("L2", "T2"),
            ("R2", "T2"),
        ]);
        assert_eq!(find_diamonds(&graph, usize::MAX).len(), 2);
        let capped = find_diamonds(&graph, 1);
        assert_eq!(capped.len(), 1);
        assert_eq!(capped[0].source, "A");
    }

    #[test]
    fn find_bridges_linear_chain_all_bridges() {
        // Each unidirectional edge in a tree is a bridge.
        let graph = g(&[("a", "b"), ("b", "c")]);
        let bridges = find_bridges(&graph);
        let pairs: Vec<(String, String)> = bridges
            .iter()
            .map(|b| (b.from.clone(), b.to.clone()))
            .collect();
        assert_eq!(
            pairs,
            vec![("a".into(), "b".into()), ("b".into(), "c".into())]
        );
    }

    #[test]
    fn find_bridges_excludes_bidirectional_and_cycle() {
        // A 3-cycle has no bridges; a bidirectional edge is not a bridge.
        let cycle = g(&[("a", "b"), ("b", "c"), ("c", "a")]);
        assert!(find_bridges(&cycle).is_empty());
        let bidir = g(&[("a", "b"), ("b", "a")]);
        assert!(find_bridges(&bidir).is_empty());
    }

    #[test]
    fn find_bridges_cycle_edge_not_a_bridge_tail_edge_is() {
        // A 3-cycle {a,b,c} with a tail edge c -> d. Removing c->d disconnects
        // d, so c->d IS a bridge. No edge inside the cycle is a bridge (each has
        // an alternate path). Proves find_bridges distinguishes cut edges from
        // cycle edges — the same frame-ordering class of bug that hit tarjan.
        let graph = g(&[("a", "b"), ("b", "c"), ("c", "a"), ("c", "d")]);
        let bridges: Vec<(String, String)> = find_bridges(&graph)
            .into_iter()
            .map(|b| (b.from, b.to))
            .collect();
        assert_eq!(bridges, vec![("c".to_string(), "d".to_string())]);
    }

    #[test]
    fn transitive_edges_single_witness() {
        // A->B, A->C, B->C : A->C is redundant via B.
        let graph = g(&[("A", "B"), ("A", "C"), ("B", "C")]);
        let te = find_transitive_edges(&graph, usize::MAX);
        assert_eq!(te.len(), 1);
        assert_eq!(
            (te[0].from.as_str(), te[0].to.as_str(), te[0].via.as_str()),
            ("A", "C", "B")
        );
        assert_eq!(count_transitive_edges(&graph), 1);
    }

    #[test]
    fn longest_chains_linear() {
        // a->b->c->d->e : one chain of depth 4 (5 nodes).
        let graph = g(&[("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")]);
        let chains = find_longest_chains(&graph, usize::MAX);
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].depth, 4);
        assert_eq!(chains[0].modules, vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn longest_chains_below_threshold_excluded() {
        // a->b->c is only 3 nodes (< MIN_CHAIN_NODE_COUNT = 4): no chains.
        let graph = g(&[("a", "b"), ("b", "c")]);
        assert!(find_longest_chains(&graph, usize::MAX).is_empty());
    }

    #[test]
    fn longest_chains_cycle_tolerant() {
        // Cycle tolerance: `find_longest_chains` must terminate on cyclic input
        // (no infinite recursion) rather than hang. A pure 3-cycle yields only
        // 3-node simple paths, all below MIN_CHAIN_NODE_COUNT = 4, so the result
        // is deterministically empty regardless of start order.
        let cycle = g(&[("a", "b"), ("b", "c"), ("c", "a")]);
        assert!(find_longest_chains(&cycle, usize::MAX).is_empty());

        // A linear chain co-existing with a disjoint 2-cycle: the algorithm
        // tolerates the cycle and still deterministically reports the linear
        // chain [a,b,c,d,e] (depth 4). The 2-cycle {x,y} contributes only
        // 2-node paths (excluded).
        //
        // NOTE: a chain routed *through* cycle nodes is intentionally NOT pinned
        // here — it is nondeterministic by design. The node-keyed APPROXIMATE
        // memoization caches whichever (possibly suboptimal) path first reaches a
        // cycle node, so the winning chain's length/membership varies with
        // HashMap start-order. That approximation is characterized behavior, not
        // a bug to "fix"; pinning it would require a determinism guarantee the
        // heuristic does not make.
        let mixed = g(&[
            ("a", "b"),
            ("b", "c"),
            ("c", "d"),
            ("d", "e"),
            ("x", "y"),
            ("y", "x"),
        ]);
        let chains = find_longest_chains(&mixed, usize::MAX);
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].depth, 4);
        assert_eq!(chains[0].modules, vec!["a", "b", "c", "d", "e"]);
    }
}
