//! Architectural metrics: coupling, cycles, layering, hubs.
//!
//! Extracted pure algorithms and supporting types for architecture analysis.
//! Report structs and OutputFormatter impls live in the `normalize` crate.

use normalize_facts::FileIndex;
pub use normalize_graph::{ImportChain, find_longest_chains};
use normalize_languages::is_programming_language;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

// ── Supporting types ─────────────────────────────────────────────────────────

/// A circular dependency cycle.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Cycle {
    /// Modules involved in the cycle.
    pub modules: Vec<String>,
}

/// Coupling metrics for a module (file).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleCoupling {
    pub path: String,
    /// Number of modules that import this one.
    pub fan_in: usize,
    /// Number of modules this one imports.
    pub fan_out: usize,
    /// Instability metric: fan_out / (fan_in + fan_out).
    /// 0 = stable (many depend on it), 1 = unstable (depends on many).
    pub instability: f64,
}

/// Symbol-level metrics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SymbolMetrics {
    pub file: String,
    pub name: String,
    pub kind: String,
    /// Number of call sites.
    pub callers: usize,
}

/// Bidirectional coupling between two modules.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CrossImport {
    pub module_a: String,
    pub module_b: String,
    /// Imports from A to B.
    pub a_to_b: usize,
    /// Imports from B to A.
    pub b_to_a: usize,
}

/// Orphan module (never imported).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OrphanModule {
    pub path: String,
    pub symbols: usize,
}

/// Hub module (high fan-in AND high fan-out).
/// These are architectural bottlenecks — everything flows through them.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HubModule {
    pub path: String,
    pub fan_in: usize,
    pub fan_out: usize,
    /// Product of fan_in * fan_out — higher = more central.
    pub hub_score: usize,
}

/// Import flow between directory layers.
/// Shows which directories import from which, helping identify layer violations.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LayerFlow {
    /// Source directory/layer.
    pub from_layer: String,
    /// Target directory/layer.
    pub to_layer: String,
    /// Number of imports in this direction.
    pub count: usize,
}

// ── Import graph ─────────────────────────────────────────────────────────────

/// Import graph: maps of who imports whom and who is imported by whom.
pub struct ImportGraph {
    pub imports_by_file: HashMap<String, HashSet<String>>,
    pub importers_by_file: HashMap<String, HashSet<String>>,
    pub raw_import_count: usize,
}

/// Build an import graph from the index.
pub async fn build_import_graph(idx: &FileIndex) -> Result<ImportGraph, libsql::Error> {
    let mut imports_by_file: HashMap<String, HashSet<String>> = HashMap::new();
    let mut importers_by_file: HashMap<String, HashSet<String>> = HashMap::new();
    let mut unresolved = 0usize;

    let conn = idx.connection();
    let stmt = conn
        .prepare("SELECT file, module, name FROM imports")
        .await?;
    let mut rows = stmt.query(()).await?;

    let mut raw_imports: Vec<(String, String)> = Vec::new();
    while let Some(row) = rows.next().await? {
        let file: String = row.get(0)?;
        let module: Option<String> = row.get(1)?;
        let name: String = row.get(2)?;

        let full_module = match module {
            Some(m) if !m.is_empty() => {
                if m.contains("::") {
                    m
                } else if m == "crate" || m == "super" || m == "self" {
                    format!("{}::{}", m, name)
                } else {
                    m
                }
            }
            _ => {
                if let Some(pos) = name.rfind("::") {
                    name[..pos].to_string()
                } else {
                    continue;
                }
            }
        };

        raw_imports.push((file, full_module));
    }

    for (source_file, module) in &raw_imports {
        let resolved_files = idx.module_to_files(module, source_file).await;

        if resolved_files.is_empty() {
            unresolved += 1;
            continue;
        }

        for target_file in resolved_files {
            imports_by_file
                .entry(source_file.clone())
                .or_default()
                .insert(target_file.clone());
            importers_by_file
                .entry(target_file)
                .or_default()
                .insert(source_file.clone());
        }
    }

    let _ = if raw_imports.is_empty() {
        0.0
    } else {
        let resolved = raw_imports.len() - unresolved;
        (resolved as f64 / raw_imports.len() as f64) * 100.0
    };

    Ok(ImportGraph {
        imports_by_file,
        importers_by_file,
        raw_import_count: raw_imports.len(),
    })
}

// ── Coupling and hub detection ────────────────────────────────────────────────

/// Result of coupling and hub detection.
pub struct CouplingAndHubs {
    pub coupling: Vec<ModuleCoupling>,
    pub hubs: Vec<HubModule>,
}

/// Compute coupling metrics and hub modules from the import graph.
pub fn compute_coupling_and_hubs(
    imports_by_file: &HashMap<String, HashSet<String>>,
    importers_by_file: &HashMap<String, HashSet<String>>,
    all_files: &HashSet<String>,
) -> CouplingAndHubs {
    let mut coupling: Vec<ModuleCoupling> = Vec::new();
    for file in all_files {
        let fan_out = imports_by_file.get(file).map(|s| s.len()).unwrap_or(0);
        let fan_in = importers_by_file.get(file).map(|s| s.len()).unwrap_or(0);
        let total = fan_in + fan_out;
        let instability = if total > 0 {
            fan_out as f64 / total as f64
        } else {
            0.5
        };

        if fan_in > 0 || fan_out > 0 {
            coupling.push(ModuleCoupling {
                path: file.clone(),
                fan_in,
                fan_out,
                instability,
            });
        }
    }

    coupling.sort_by(|a, b| b.fan_in.cmp(&a.fan_in));

    let mut hub_modules: Vec<HubModule> = coupling
        .iter()
        .filter(|m| m.fan_in >= 3 && m.fan_out >= 3)
        .map(|m| HubModule {
            path: m.path.clone(),
            fan_in: m.fan_in,
            fan_out: m.fan_out,
            hub_score: m.fan_in * m.fan_out,
        })
        .collect();
    hub_modules.sort_by(|a, b| b.hub_score.cmp(&a.hub_score));
    hub_modules.truncate(10);

    coupling.truncate(15);
    CouplingAndHubs {
        coupling,
        hubs: hub_modules,
    }
}

// ── Cross-import detection ────────────────────────────────────────────────────

/// Detect bidirectional coupling between module pairs.
pub fn detect_cross_imports(
    imports_by_file: &HashMap<String, HashSet<String>>,
) -> Vec<CrossImport> {
    let mut cross_imports: Vec<CrossImport> = Vec::new();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();

    for (file_a, imports_a) in imports_by_file {
        for file_b in imports_a {
            if let Some(imports_b) = imports_by_file.get(file_b)
                && imports_b.contains(file_a)
            {
                let pair = if file_a < file_b {
                    (file_a.clone(), file_b.clone())
                } else {
                    (file_b.clone(), file_a.clone())
                };
                if !seen_pairs.contains(&pair) {
                    seen_pairs.insert(pair);
                    let a_to_b = imports_a.iter().filter(|f| *f == file_b).count();
                    let b_to_a = imports_b.iter().filter(|f| *f == file_a).count();
                    cross_imports.push(CrossImport {
                        module_a: file_a.clone(),
                        module_b: file_b.clone(),
                        a_to_b,
                        b_to_a,
                    });
                }
            }
        }
    }
    cross_imports
}

// ── Orphan module detection ───────────────────────────────────────────────────

/// Find modules that have no importers (and are not entry-point files).
pub async fn find_orphan_modules(
    conn: &libsql::Connection,
    importers_by_file: &HashMap<String, HashSet<String>>,
) -> Result<Vec<OrphanModule>, libsql::Error> {
    let mut orphans: Vec<OrphanModule> = Vec::new();
    let stmt = conn
        .prepare("SELECT file, COUNT(*) FROM symbols GROUP BY file")
        .await?;
    let mut rows = stmt.query(()).await?;
    while let Some(row) = rows.next().await? {
        let file: String = row.get(0)?;
        let symbol_count: i64 = row.get(1)?;

        if !is_programming_language(Path::new(&file)) {
            continue;
        }

        let is_imported = importers_by_file.contains_key(&file);
        let is_likely_entry = file.contains("main.")
            || file.contains("mod.rs")
            || file.contains("lib.rs")
            || file.contains("index.")
            || file.contains("__init__")
            || file.contains("test")
            || file.contains("spec");

        if !is_imported && !is_likely_entry && symbol_count > 0 {
            let symbols = if symbol_count < 0 {
                0usize
            } else {
                symbol_count as usize
            };
            orphans.push(OrphanModule {
                path: file,
                symbols,
            });
        }
    }
    orphans.truncate(10);
    Ok(orphans)
}

// ── Symbol hotspot detection ──────────────────────────────────────────────────

const GENERIC_METHODS: &[&str] = &[
    "new",
    "default",
    "from",
    "into",
    "clone",
    "to_string",
    "as_str",
    "as_ref",
    "get",
    "set",
    "len",
    "is_empty",
    "iter",
    "next",
    "unwrap",
    "expect",
    "ok",
    "err",
    "some",
    "none",
    "push",
    "pop",
    "insert",
    "remove",
    "contains",
    "map",
    "filter",
    "collect",
    "fmt",
    "write",
    "read",
    "Ok",
    "Err",
    "Some",
    "None",
];

/// Find symbols imported from many places (symbol hotspots).
pub async fn find_symbol_hotspots(
    conn: &libsql::Connection,
) -> Result<Vec<SymbolMetrics>, libsql::Error> {
    let generic: HashSet<&str> = GENERIC_METHODS.iter().copied().collect();

    let mut symbol_callers: HashMap<String, usize> = HashMap::new();
    let stmt = conn
        .prepare("SELECT callee_name, COUNT(*) as cnt FROM calls GROUP BY callee_name ORDER BY cnt DESC LIMIT 100")
        .await?;
    let mut rows = stmt.query(()).await?;
    while let Some(row) = rows.next().await? {
        let name: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        if !generic.contains(name.as_str()) {
            let n = if count < 0 { 0usize } else { count as usize };
            symbol_callers.insert(name, n);
        }
    }

    // Filter to callers > 3 before the lookup query.
    let candidates: Vec<(String, usize)> =
        symbol_callers.into_iter().filter(|(_, c)| *c > 3).collect();

    let mut hotspots: Vec<SymbolMetrics> = Vec::new();
    if !candidates.is_empty() {
        let placeholders = candidates
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT name, file, kind FROM symbols WHERE name IN ({}) GROUP BY name",
            placeholders
        );
        let stmt = conn.prepare(&sql).await?;
        let params: Vec<libsql::Value> = candidates
            .iter()
            .map(|(n, _)| libsql::Value::Text(n.clone()))
            .collect();
        let mut rows = stmt.query(params).await?;
        // Build a lookup from name → callers count.
        let callers_map: HashMap<String, usize> = candidates.into_iter().collect();
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            let file: String = row.get(1)?;
            let kind: String = row.get(2)?;
            if let Some(&callers) = callers_map.get(&name) {
                hotspots.push(SymbolMetrics {
                    file,
                    name,
                    kind,
                    callers,
                });
            }
        }
    }
    hotspots.sort_by(|a, b| b.callers.cmp(&a.callers));
    hotspots.truncate(10);
    Ok(hotspots)
}

// ── Cycle detection ───────────────────────────────────────────────────────────

/// Find cycles in the import graph using iterative DFS.
pub fn find_cycles(graph: &HashMap<String, HashSet<String>>) -> Vec<Cycle> {
    let mut cycles = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();

    for start in graph.keys() {
        if visited.contains(start) {
            continue;
        }
        // Each stack frame: (node, iterator-index-into-neighbors, already-pushed-to-path)
        // We use an explicit stack of (node, neighbor_index) pairs.
        // rec_stack tracks the current DFS path as a set; path tracks it as an ordered Vec.
        let mut rec_stack: HashSet<String> = HashSet::new();
        let mut path: Vec<String> = Vec::new();
        // Stack entries: (node_name, index_of_next_neighbor_to_visit)
        let mut stack: Vec<(String, usize)> = Vec::new();

        // Push the start node
        visited.insert(start.clone());
        rec_stack.insert(start.clone());
        path.push(start.clone());
        stack.push((start.clone(), 0));

        while let Some((node, idx)) = stack.last_mut() {
            let node = node.clone();
            let neighbors: Vec<String> = graph
                .get(&node)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();

            if *idx < neighbors.len() {
                let neighbor = neighbors[*idx].clone();
                *idx += 1;

                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    rec_stack.insert(neighbor.clone());
                    path.push(neighbor.clone());
                    stack.push((neighbor, 0));
                } else if rec_stack.contains(&neighbor)
                    && let Some(pos) = path.iter().position(|x| x == &neighbor)
                    && path[pos..].len() > 1
                {
                    cycles.push(Cycle {
                        modules: path[pos..].to_vec(),
                    });
                }
            } else {
                // Done with this node — pop it
                stack.pop();
                path.pop();
                rec_stack.remove(&node);
            }
        }
    }

    // Deduplicate cycles (same cycle can be found starting from different nodes).
    let mut unique_cycles: Vec<Cycle> = Vec::new();
    let mut seen_cycle_sets: HashSet<Vec<String>> = HashSet::new();

    for cycle in cycles {
        let mut sorted = cycle.modules.clone();
        sorted.sort();
        if !seen_cycle_sets.contains(&sorted) {
            seen_cycle_sets.insert(sorted);
            unique_cycles.push(cycle);
        }
    }

    unique_cycles.truncate(10);
    unique_cycles
}

// ── Longest chain detection ───────────────────────────────────────────────────

/// Find the longest import chains (dependency paths) in the graph.
/// Uses DFS to find the longest path from each node, avoiding cycles.
pub fn find_longest_chains(graph: &HashMap<String, HashSet<String>>) -> Vec<ImportChain> {
    let mut longest_paths: Vec<ImportChain> = Vec::new();
    let mut memo: HashMap<String, Vec<String>> = HashMap::new();

    for start in graph.keys() {
        let mut visited: HashSet<String> = HashSet::new();
        let path = longest_path_from(start, graph, &mut visited, &mut memo);
        if path.len() > 3 {
            longest_paths.push(ImportChain {
                depth: path.len() - 1,
                modules: path,
            });
        }
    }

    longest_paths.sort_by(|a, b| b.depth.cmp(&a.depth));

    let mut unique_chains: Vec<ImportChain> = Vec::new();
    for chain in longest_paths {
        let dominated = unique_chains.iter().any(|existing| {
            existing.modules.len() > chain.modules.len()
                && existing.modules.ends_with(&chain.modules)
        });
        if !dominated {
            unique_chains.push(chain);
        }
        if unique_chains.len() >= 5 {
            break;
        }
    }

    unique_chains
}

/// Find the longest path from a node using DFS with memoization.
fn longest_path_from(
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

// ── Layer extraction ──────────────────────────────────────────────────────────

/// Extract the layer (top-level directory) from a file path.
/// Returns the first significant directory component.
pub fn extract_layer(path: &str) -> String {
    let path = path
        .strip_prefix("crates/normalize/src/")
        .or_else(|| path.strip_prefix("crates/normalize-"))
        .or_else(|| path.strip_prefix("src/"))
        .unwrap_or(path);

    if let Some(pos) = path.find('/') {
        path[..pos].to_string()
    } else {
        path.split('.').next().unwrap_or("root").to_string()
    }
}

/// Compute import flows between directory layers.
pub fn compute_layer_flows(graph: &HashMap<String, HashSet<String>>) -> Vec<LayerFlow> {
    let mut flow_counts: HashMap<(String, String), usize> = HashMap::new();

    for (from_file, to_files) in graph {
        let from_layer = extract_layer(from_file);
        for to_file in to_files {
            let to_layer = extract_layer(to_file);
            if from_layer != to_layer {
                *flow_counts
                    .entry((from_layer.clone(), to_layer.clone()))
                    .or_insert(0) += 1;
            }
        }
    }

    let mut flows: Vec<LayerFlow> = flow_counts
        .into_iter()
        .map(|((from, to), count)| LayerFlow {
            from_layer: from,
            to_layer: to,
            count,
        })
        .collect();

    flows.sort_by(|a, b| b.count.cmp(&a.count));
    flows.truncate(15);
    flows
}

// ── Depth computation ─────────────────────────────────────────────────────────

/// Compute depth for a single node via DFS + memoization.
/// depth(M) = max(1 + depth(importer) for importer in importers_by_file[M]), base 0.
pub fn compute_depth(
    node: &str,
    importers_by_file: &HashMap<String, HashSet<String>>,
    memo: &mut HashMap<String, usize>,
    in_stack: &mut HashSet<String>,
) -> usize {
    if let Some(&d) = memo.get(node) {
        return d;
    }
    if in_stack.contains(node) {
        return 0;
    }
    in_stack.insert(node.to_string());

    let depth = match importers_by_file.get(node) {
        None => 0,
        Some(importers) if importers.is_empty() => 0,
        Some(importers) => importers
            .iter()
            .map(|imp| 1 + compute_depth(imp, importers_by_file, memo, in_stack))
            .max()
            .unwrap_or(0),
    };

    in_stack.remove(node);
    memo.insert(node.to_string(), depth);
    depth
}

/// Compute downstream count for a node: BFS through importers_by_file.
pub fn compute_downstream(
    node: &str,
    importers_by_file: &HashMap<String, HashSet<String>>,
) -> usize {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    if let Some(importers) = importers_by_file.get(node) {
        for imp in importers {
            if visited.insert(imp.clone()) {
                queue.push_back(imp.clone());
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        if let Some(importers) = importers_by_file.get(&current) {
            for imp in importers {
                if visited.insert(imp.clone()) {
                    queue.push_back(imp.clone());
                }
            }
        }
    }

    visited.len()
}

// ── Layering compliance ───────────────────────────────────────────────────────

/// Per-module layering metrics returned by `compute_layering_compliance`.
pub struct LayeringModuleResult {
    pub module: String,
    pub layer: String,
    /// Cross-layer imports (downward + upward only).
    pub total_imports: usize,
    pub downward_imports: usize,
    pub upward_imports: usize,
    /// Imports within the same layer.
    pub self_imports: usize,
    /// downward / (downward + upward); 1.0 if no cross-layer imports.
    pub compliance: f64,
}

/// Classify imports for each module as downward, upward, or self-layer.
///
/// Takes the resolved import graph and per-layer average depths.
/// Returns per-module compliance entries sorted worst-first.
pub fn compute_layering_compliance(
    imports_by_file: &HashMap<String, HashSet<String>>,
    all_modules: &HashSet<String>,
    layer_avg_depth: &HashMap<String, f64>,
) -> Vec<LayeringModuleResult> {
    let mut entries: Vec<LayeringModuleResult> = Vec::new();

    for module in all_modules {
        let imports = match imports_by_file.get(module) {
            Some(targets) => targets,
            None => continue,
        };

        let src_layer = extract_layer(module);
        let src_avg = layer_avg_depth.get(&src_layer).copied().unwrap_or(0.0);

        let mut downward = 0usize;
        let mut upward = 0usize;
        let mut self_count = 0usize;

        for target in imports {
            let tgt_layer = extract_layer(target);
            if tgt_layer == src_layer {
                self_count += 1;
            } else {
                let tgt_avg = layer_avg_depth.get(&tgt_layer).copied().unwrap_or(0.0);
                if tgt_avg > src_avg {
                    downward += 1;
                } else if tgt_avg < src_avg {
                    upward += 1;
                } else {
                    self_count += 1;
                }
            }
        }

        let cross = downward + upward;
        let compliance = if cross == 0 {
            1.0
        } else {
            downward as f64 / cross as f64
        };

        entries.push(LayeringModuleResult {
            module: module.clone(),
            layer: src_layer,
            total_imports: cross,
            downward_imports: downward,
            upward_imports: upward,
            self_imports: self_count,
            compliance,
        });
    }

    entries.sort_by(|a, b| {
        a.compliance
            .partial_cmp(&b.compliance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.upward_imports.cmp(&a.upward_imports))
            .then(a.module.cmp(&b.module))
    });

    entries
}
