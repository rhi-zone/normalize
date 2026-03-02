//! Graph-theoretic metrics on the module dependency graph.
//!
//! Computes structural properties that existing commands don't cover:
//! strongly connected components (Tarjan's), diamond dependencies,
//! bridge edges, transitive (redundant) imports, and overall graph density.

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

/// Overall graph statistics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GraphStats {
    pub nodes: usize,
    pub edges: usize,
    /// Edge density: edges / (nodes × (nodes − 1))
    pub density: f64,
    pub weakly_connected_components: usize,
    pub largest_component_size: usize,
    pub scc_count: usize,
    /// SCCs with more than one module (actual circular-dependency clusters)
    pub nontrivial_scc_count: usize,
    pub diamond_count: usize,
    pub bridge_count: usize,
    pub transitive_edge_count: usize,
}

/// A strongly connected component (circular-dependency cluster).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Scc {
    /// Modules in the SCC
    pub modules: Vec<String>,
    /// Number of edges within the SCC
    pub internal_edges: usize,
}

/// A diamond dependency: source imports left and right, both import target.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Diamond {
    pub source: String,
    pub left: String,
    pub right: String,
    pub target: String,
}

/// A bridge edge whose removal disconnects the graph.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BridgeEdge {
    pub from: String,
    pub to: String,
}

/// A transitive (redundant) import: A→C is redundant because A→B→C.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TransitiveEdge {
    pub from: String,
    pub to: String,
    pub via: String,
}

/// Full graph analysis report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GraphReport {
    pub stats: GraphStats,
    pub sccs: Vec<Scc>,
    pub diamonds: Vec<Diamond>,
    pub bridges: Vec<BridgeEdge>,
    pub transitive_edges: Vec<TransitiveEdge>,
}

// ---------------------------------------------------------------------------
// Algorithms
// ---------------------------------------------------------------------------

/// Collect all nodes from the import graph.
fn all_nodes(imports: &HashMap<String, HashSet<String>>) -> HashSet<String> {
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
fn edge_count(imports: &HashMap<String, HashSet<String>>) -> usize {
    imports.values().map(|s| s.len()).sum()
}

/// Weakly connected components via BFS on the undirected view.
fn weakly_connected_components(imports: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
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
fn tarjan_sccs(imports: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    let nodes = all_nodes(imports);
    let mut index_counter: usize = 0;
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

                    let neighbors: Vec<String> = imports
                        .get(&node)
                        .map(|s| s.iter().cloned().collect())
                        .unwrap_or_default();

                    // Push neighbors in reverse so we process them in order
                    for neighbor in neighbors.into_iter().rev() {
                        if !indices.contains_key(&neighbor) {
                            call_stack.push(Frame::Resume(node.clone(), neighbor.clone()));
                            call_stack.push(Frame::Enter(neighbor));
                        } else if on_stack.contains(&neighbor) {
                            let nl = *lowlink.get(&node).unwrap();
                            let ni = *indices.get(&neighbor).unwrap();
                            lowlink.insert(node.clone(), nl.min(ni));
                        }
                    }

                    // After processing all neighbors, check if this is a root
                    // We need a sentinel to know when we're done with a node
                    call_stack.push(Frame::Resume(node.clone(), String::new()));
                }
                Frame::Resume(node, neighbor) => {
                    if neighbor.is_empty() {
                        // Sentinel: all neighbors processed, check if root of SCC
                        let nl = *lowlink.get(&node).unwrap();
                        let ni = *indices.get(&node).unwrap();
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
fn find_sccs(imports: &HashMap<String, HashSet<String>>) -> Vec<Scc> {
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
    result.sort_by(|a, b| b.modules.len().cmp(&a.modules.len()));
    result
}

/// Diamond detection: for each node A with imports {B₁,B₂,...}, check pairs for shared targets.
fn find_diamonds(imports: &HashMap<String, HashSet<String>>, limit: usize) -> Vec<Diamond> {
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
fn find_bridges(imports: &HashMap<String, HashSet<String>>) -> Vec<BridgeEdge> {
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
        let ui = *node_idx.get(u.as_str()).unwrap();
        for v in vs {
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
    let mut timer: usize = 1;
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
fn find_transitive_edges(
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
fn count_transitive_edges(imports: &HashMap<String, HashSet<String>>) -> usize {
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

// ---------------------------------------------------------------------------
// Main analysis
// ---------------------------------------------------------------------------

/// Analyze graph-theoretic properties of the module dependency graph.
pub async fn analyze_graph(idx: &FileIndex, limit: usize) -> Result<GraphReport, libsql::Error> {
    use super::architecture::build_import_graph;

    let graph = build_import_graph(idx).await?;
    let imports = &graph.imports_by_file;

    let nodes = all_nodes(imports);
    let node_count = nodes.len();
    let edges = edge_count(imports);
    let density = if node_count > 1 {
        edges as f64 / (node_count as f64 * (node_count as f64 - 1.0))
    } else {
        0.0
    };

    let wcc = weakly_connected_components(imports);
    let wcc_count = wcc.len();
    let largest_component = wcc.first().map(|c| c.len()).unwrap_or(0);

    let all_sccs = tarjan_sccs(imports);
    let scc_count = all_sccs.len();
    let mut sccs = find_sccs(imports);
    let nontrivial_scc_count = sccs.len();

    let mut diamonds = find_diamonds(
        imports,
        if limit == usize::MAX {
            usize::MAX
        } else {
            limit * 10
        },
    );
    let diamond_count = diamonds.len();

    let bridges = find_bridges(imports);
    let bridge_count = bridges.len();

    let transitive_edge_count = count_transitive_edges(imports);
    let mut transitive_edges = find_transitive_edges(
        imports,
        if limit == usize::MAX {
            usize::MAX
        } else {
            limit
        },
    );

    // Apply limits
    sccs.truncate(limit);
    diamonds.truncate(limit);
    transitive_edges.truncate(limit);

    let stats = GraphStats {
        nodes: node_count,
        edges,
        density,
        weakly_connected_components: wcc_count,
        largest_component_size: largest_component,
        scc_count,
        nontrivial_scc_count,
        diamond_count,
        bridge_count,
        transitive_edge_count,
    };

    Ok(GraphReport {
        stats,
        sccs,
        diamonds,
        bridges,
        transitive_edges,
    })
}

/// CLI entry point (sync wrapper).
pub fn analyze_graph_sync(root: &Path, limit: usize) -> Result<GraphReport, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create async runtime: {}", e))?;

    rt.block_on(async {
        let idx = crate::index::ensure_ready(root).await?;
        analyze_graph(&idx, limit)
            .await
            .map_err(|e| format!("Graph analysis failed: {}", e))
    })
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - (max_len - 3)..])
    }
}

impl OutputFormatter for GraphReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();
        let s = &self.stats;

        out.push(format!(
            "# Graph — {} nodes, {} edges, density {:.3}",
            s.nodes, s.edges, s.density
        ));
        out.push(format!(
            "  {} weakly connected components (largest: {})",
            s.weakly_connected_components, s.largest_component_size
        ));
        out.push(format!(
            "  {} circular-dependency clusters, {} diamonds, {} bridges, {} transitive edges",
            s.nontrivial_scc_count, s.diamond_count, s.bridge_count, s.transitive_edge_count
        ));
        out.push(String::new());

        if s.nodes == 0 {
            out.push("No import data found. Run `normalize facts rebuild` first.".to_string());
            return out.join("\n");
        }

        // SCCs
        if !self.sccs.is_empty() {
            out.push(format!(
                "## Circular dependency clusters ({} SCCs)",
                self.sccs.len()
            ));
            for scc in &self.sccs {
                let modules: Vec<String> =
                    scc.modules.iter().map(|m| truncate_path(m, 40)).collect();
                out.push(format!(
                    "  [{} modules, {} edges] {}",
                    scc.modules.len(),
                    scc.internal_edges,
                    modules.join(", ")
                ));
            }
            out.push(String::new());
        }

        // Diamonds
        if !self.diamonds.is_empty() {
            out.push(format!(
                "## Diamond dependencies ({} found)",
                self.stats.diamond_count
            ));
            for d in &self.diamonds {
                out.push(format!(
                    "  {} → {{{}, {}}} → {}",
                    truncate_path(&d.source, 30),
                    truncate_path(&d.left, 25),
                    truncate_path(&d.right, 25),
                    truncate_path(&d.target, 30),
                ));
            }
            out.push(String::new());
        }

        // Bridges
        if !self.bridges.is_empty() {
            out.push(format!(
                "## Bridge edges ({} critical dependencies)",
                self.bridges.len()
            ));
            for b in &self.bridges {
                out.push(format!(
                    "  {} → {}",
                    truncate_path(&b.from, 40),
                    truncate_path(&b.to, 40),
                ));
            }
            out.push(String::new());
        }

        // Transitive edges
        if !self.transitive_edges.is_empty() {
            let showing = if self.transitive_edges.len() < self.stats.transitive_edge_count {
                format!(" (showing {})", self.transitive_edges.len())
            } else {
                String::new()
            };
            out.push(format!(
                "## Transitive edges ({} redundant{})",
                self.stats.transitive_edge_count, showing
            ));
            for te in &self.transitive_edges {
                out.push(format!(
                    "  {} → {}  (via {})",
                    truncate_path(&te.from, 30),
                    truncate_path(&te.to, 30),
                    truncate_path(&te.via, 30),
                ));
            }
            out.push(String::new());
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut out = Vec::new();
        let s = &self.stats;

        out.push(format!(
            "\x1b[1;36m# Graph\x1b[0m — \x1b[1m{}\x1b[0m nodes, \x1b[1m{}\x1b[0m edges, density \x1b[33m{:.3}\x1b[0m",
            s.nodes, s.edges, s.density
        ));
        out.push(format!(
            "  \x1b[32m{}\x1b[0m weakly connected components (largest: \x1b[1m{}\x1b[0m)",
            s.weakly_connected_components, s.largest_component_size
        ));

        let scc_color = if s.nontrivial_scc_count > 0 {
            "\x1b[1;31m"
        } else {
            "\x1b[32m"
        };
        let diamond_color = if s.diamond_count > 0 {
            "\x1b[33m"
        } else {
            "\x1b[32m"
        };
        let bridge_color = if s.bridge_count > 0 {
            "\x1b[1;33m"
        } else {
            "\x1b[32m"
        };
        let trans_color = if s.transitive_edge_count > 0 {
            "\x1b[33m"
        } else {
            "\x1b[32m"
        };

        out.push(format!(
            "  {}{}\x1b[0m circular-dependency clusters, {}{}\x1b[0m diamonds, {}{}\x1b[0m bridges, {}{}\x1b[0m transitive edges",
            scc_color, s.nontrivial_scc_count,
            diamond_color, s.diamond_count,
            bridge_color, s.bridge_count,
            trans_color, s.transitive_edge_count,
        ));
        out.push(String::new());

        if s.nodes == 0 {
            out.push("No import data found. Run `normalize facts rebuild` first.".to_string());
            return out.join("\n");
        }

        // SCCs
        if !self.sccs.is_empty() {
            out.push(format!(
                "\x1b[1;31m## Circular dependency clusters ({} SCCs)\x1b[0m",
                self.sccs.len()
            ));
            for scc in &self.sccs {
                let modules: Vec<String> =
                    scc.modules.iter().map(|m| truncate_path(m, 40)).collect();
                out.push(format!(
                    "  \x1b[31m[{} modules, {} edges]\x1b[0m {}",
                    scc.modules.len(),
                    scc.internal_edges,
                    modules.join(", ")
                ));
            }
            out.push(String::new());
        }

        // Diamonds
        if !self.diamonds.is_empty() {
            out.push(format!(
                "\x1b[1;33m## Diamond dependencies ({} found)\x1b[0m",
                self.stats.diamond_count
            ));
            for d in &self.diamonds {
                out.push(format!(
                    "  {} \x1b[33m→\x1b[0m {{{}, {}}} \x1b[33m→\x1b[0m {}",
                    truncate_path(&d.source, 30),
                    truncate_path(&d.left, 25),
                    truncate_path(&d.right, 25),
                    truncate_path(&d.target, 30),
                ));
            }
            out.push(String::new());
        }

        // Bridges
        if !self.bridges.is_empty() {
            out.push(format!(
                "\x1b[1;33m## Bridge edges ({} critical dependencies)\x1b[0m",
                self.bridges.len()
            ));
            for b in &self.bridges {
                out.push(format!(
                    "  {} \x1b[1;33m→\x1b[0m {}",
                    truncate_path(&b.from, 40),
                    truncate_path(&b.to, 40),
                ));
            }
            out.push(String::new());
        }

        // Transitive edges
        if !self.transitive_edges.is_empty() {
            let showing = if self.transitive_edges.len() < self.stats.transitive_edge_count {
                format!(" (showing {})", self.transitive_edges.len())
            } else {
                String::new()
            };
            out.push(format!(
                "\x1b[33m## Transitive edges ({} redundant{})\x1b[0m",
                self.stats.transitive_edge_count, showing
            ));
            for te in &self.transitive_edges {
                out.push(format!(
                    "  {} → {}  \x1b[2m(via {})\x1b[0m",
                    truncate_path(&te.from, 30),
                    truncate_path(&te.to, 30),
                    truncate_path(&te.via, 30),
                ));
            }
            out.push(String::new());
        }

        out.join("\n")
    }
}
