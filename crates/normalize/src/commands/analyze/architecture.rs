//! Architecture analysis: coupling, cycles, and structural insights
//!
//! Provides insights by default - no configuration needed.

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use normalize_languages::is_programming_language;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A circular dependency cycle
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Cycle {
    /// Modules involved in the cycle
    pub modules: Vec<String>,
}

/// Coupling metrics for a module (file)
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleCoupling {
    pub path: String,
    /// Number of modules that import this one
    pub fan_in: usize,
    /// Number of modules this one imports
    pub fan_out: usize,
    /// Instability metric: fan_out / (fan_in + fan_out)
    /// 0 = stable (many depend on it), 1 = unstable (depends on many)
    pub instability: f64,
}

/// Symbol-level metrics
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SymbolMetrics {
    pub file: String,
    pub name: String,
    pub kind: String,
    /// Number of call sites
    pub callers: usize,
}

/// Bidirectional coupling between two modules
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CrossImport {
    pub module_a: String,
    pub module_b: String,
    /// Imports from A to B
    pub a_to_b: usize,
    /// Imports from B to A
    pub b_to_a: usize,
}

/// Orphan module (never imported)
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct OrphanModule {
    pub path: String,
    pub symbols: usize,
}

/// Hub module (high fan-in AND high fan-out)
/// These are architectural bottlenecks - everything flows through them.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HubModule {
    pub path: String,
    pub fan_in: usize,
    pub fan_out: usize,
    /// Product of fan_in * fan_out - higher = more central
    pub hub_score: usize,
}

/// A deep import chain (longest dependency path)
/// Long chains can indicate layering issues or overly deep hierarchies.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ImportChain {
    /// Modules in the chain from start to end
    pub modules: Vec<String>,
    /// Length of the chain (number of edges, not nodes)
    pub depth: usize,
}

/// Import flow between directory layers.
/// Shows which directories import from which, helping identify layer violations.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LayerFlow {
    /// Source directory/layer
    pub from_layer: String,
    /// Target directory/layer
    pub to_layer: String,
    /// Number of imports in this direction
    pub count: usize,
}

/// Full architecture analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ArchitectureReport {
    pub cycles: Vec<Cycle>,
    pub cross_imports: Vec<CrossImport>,
    pub hub_modules: Vec<HubModule>,
    pub deep_chains: Vec<ImportChain>,
    pub layer_flows: Vec<LayerFlow>,
    pub coupling_hotspots: Vec<ModuleCoupling>,
    pub orphan_modules: Vec<OrphanModule>,
    pub symbol_hotspots: Vec<SymbolMetrics>,
    pub total_modules: usize,
    pub total_symbols: usize,
    pub total_imports: usize,
    pub resolved_imports: usize,
}

impl OutputFormatter for ArchitectureReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        // Circular Dependencies
        lines.push("## Circular Dependencies".to_string());
        if self.cycles.is_empty() {
            lines.push("  None detected ✓".to_string());
        } else {
            for cycle in &self.cycles {
                let path = cycle.modules.join(" → ");
                lines.push(format!("  {} → {}", path, cycle.modules[0]));
            }
        }
        lines.push(String::new());

        // Cross-imports (bidirectional coupling)
        lines.push("## Cross-Imports (bidirectional coupling)".to_string());
        if self.cross_imports.is_empty() {
            lines.push("  None detected ✓".to_string());
        } else {
            for ci in &self.cross_imports {
                lines.push(format!(
                    "  {} ↔ {}  ({} / {} imports)",
                    ci.module_a, ci.module_b, ci.a_to_b, ci.b_to_a
                ));
            }
        }
        lines.push(String::new());

        // Hub modules
        if !self.hub_modules.is_empty() {
            lines.push("## Hub Modules (high fan-in AND fan-out)".to_string());
            lines.push(format!(
                "  {:<50} {:>6} {:>7} {:>10}",
                "Module", "Fan-in", "Fan-out", "Hub Score"
            ));
            lines.push(format!("  {}", "-".repeat(76)));
            for h in &self.hub_modules {
                let display_path = truncate_path(&h.path, 48);
                lines.push(format!(
                    "  {:<50} {:>6} {:>7} {:>10}",
                    display_path, h.fan_in, h.fan_out, h.hub_score
                ));
            }
            lines.push(String::new());
        }

        // Deep import chains
        if !self.deep_chains.is_empty() {
            lines.push("## Deep Import Chains".to_string());
            for chain in &self.deep_chains {
                let short_modules: Vec<String> =
                    chain.modules.iter().map(|m| truncate_path(m, 30)).collect();
                lines.push(format!(
                    "  [depth {}] {}",
                    chain.depth,
                    short_modules.join(" → ")
                ));
            }
            lines.push(String::new());
        }

        // Layer flows (inter-directory imports)
        if !self.layer_flows.is_empty() {
            lines.push("## Layer Dependencies".to_string());
            lines.push(format!("  {:<20} → {:<20} {:>8}", "From", "To", "Imports"));
            lines.push(format!("  {}", "-".repeat(52)));
            for flow in &self.layer_flows {
                lines.push(format!(
                    "  {:<20} → {:<20} {:>8}",
                    flow.from_layer, flow.to_layer, flow.count
                ));
            }
            lines.push(String::new());
        }

        // Coupling hotspots
        lines.push("## Coupling Hotspots".to_string());
        lines.push(format!(
            "  {:<50} {:>6} {:>6} {:>10}",
            "Module", "Fan-in", "Fan-out", "Instability"
        ));
        lines.push(format!("  {}", "-".repeat(76)));
        for m in &self.coupling_hotspots {
            let display_path = truncate_path(&m.path, 48);
            let instability_indicator = if m.instability > 0.8 {
                " (unstable)"
            } else if m.instability < 0.2 && m.fan_in > 5 {
                " (stable)"
            } else {
                ""
            };
            lines.push(format!(
                "  {:<50} {:>6} {:>6} {:>10.2}{}",
                display_path, m.fan_in, m.fan_out, m.instability, instability_indicator
            ));
        }
        lines.push(String::new());

        // Symbol hotspots (most called)
        if !self.symbol_hotspots.is_empty() {
            lines.push("## Symbol Hotspots (most called)".to_string());
            lines.push(format!(
                "  {:<40} {:<12} {:>8}",
                "Symbol", "Kind", "Callers"
            ));
            lines.push(format!("  {}", "-".repeat(64)));
            for s in &self.symbol_hotspots {
                let display = format!("{}:{}", truncate_path(&s.file, 20), s.name);
                let display = if display.len() > 38 {
                    format!("...{}", &display[display.len() - 35..])
                } else {
                    display
                };
                lines.push(format!("  {:<40} {:<12} {:>8}", display, s.kind, s.callers));
            }
            lines.push(String::new());
        }

        // Orphan modules
        if !self.orphan_modules.is_empty() {
            lines.push("## Orphan Modules (never imported)".to_string());
            for o in &self.orphan_modules {
                lines.push(format!("  {} ({} symbols)", o.path, o.symbols));
            }
            lines.push(String::new());
        }

        // Summary
        lines.push("## Summary".to_string());
        lines.push(format!("  Modules: {}", self.total_modules));
        lines.push(format!("  Symbols: {}", self.total_symbols));
        lines.push(format!(
            "  Imports: {} total, {} resolved to local files",
            self.total_imports, self.resolved_imports
        ));
        lines.push(format!("  Circular dependencies: {}", self.cycles.len()));
        lines.push(format!("  Cross-imports: {}", self.cross_imports.len()));
        lines.push(format!("  Orphan modules: {}", self.orphan_modules.len()));

        // Note about resolution
        if self.total_imports > 0 && self.resolved_imports == 0 {
            lines.push(String::new());
            lines.push(
                "Note: No imports resolved to local files. Coupling metrics require local import resolution.".to_string(),
            );
            lines.push(
                "      External deps (std, third-party crates) don't contribute to coupling analysis."
                    .to_string(),
            );
        }

        lines.join("\n")
    }
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - (max_len - 3)..])
    }
}

/// Run architecture analysis
pub fn cmd_architecture(root: &Path, json: bool) -> i32 {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let idx = match rt.block_on(crate::index::open_if_enabled(root)) {
        Some(i) => i,
        None => {
            eprintln!("Index not available. Run `normalize index` first.");
            eprintln!("Or enable indexing: `normalize config set index.enabled true`");
            return 1;
        }
    };

    let report = match rt.block_on(analyze_architecture(&idx)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
            return 1;
        }
    };

    let config = crate::config::NormalizeConfig::load(root);
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);
    0
}

/// Import graph: maps of who imports whom and who is imported by whom.
struct ImportGraph {
    imports_by_file: HashMap<String, HashSet<String>>,
    importers_by_file: HashMap<String, HashSet<String>>,
    raw_import_count: usize,
}

async fn build_import_graph(idx: &FileIndex) -> Result<ImportGraph, libsql::Error> {
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

fn compute_coupling_and_hubs(
    imports_by_file: &HashMap<String, HashSet<String>>,
    importers_by_file: &HashMap<String, HashSet<String>>,
    all_files: &HashSet<String>,
) -> (Vec<ModuleCoupling>, Vec<HubModule>) {
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
    (coupling, hub_modules)
}

fn detect_cross_imports(imports_by_file: &HashMap<String, HashSet<String>>) -> Vec<CrossImport> {
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

async fn find_orphan_modules(
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
            orphans.push(OrphanModule {
                path: file,
                symbols: symbol_count as usize,
            });
        }
    }
    orphans.truncate(10);
    Ok(orphans)
}

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

async fn find_symbol_hotspots(
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
            symbol_callers.insert(name, count as usize);
        }
    }

    let mut hotspots: Vec<SymbolMetrics> = Vec::new();
    for (name, callers) in &symbol_callers {
        let stmt = conn
            .prepare("SELECT file, kind FROM symbols WHERE name = ? LIMIT 1")
            .await?;
        let mut rows = stmt.query([name.as_str()]).await?;
        if let Some(row) = rows.next().await? {
            let file: String = row.get(0)?;
            let kind: String = row.get(1)?;
            if *callers > 3 {
                hotspots.push(SymbolMetrics {
                    file,
                    name: name.clone(),
                    kind,
                    callers: *callers,
                });
            }
        }
    }
    hotspots.sort_by(|a, b| b.callers.cmp(&a.callers));
    hotspots.truncate(10);
    Ok(hotspots)
}

pub async fn analyze_architecture(idx: &FileIndex) -> Result<ArchitectureReport, libsql::Error> {
    let graph = build_import_graph(idx).await?;
    let conn = idx.connection();

    // Get all source files (programming languages only)
    let mut all_files: HashSet<String> = HashSet::new();
    let stmt = conn.prepare("SELECT DISTINCT file FROM symbols").await?;
    let mut rows = stmt.query(()).await?;
    while let Some(row) = rows.next().await? {
        let path: String = row.get(0)?;
        if is_programming_language(Path::new(&path)) {
            all_files.insert(path);
        }
    }

    let (coupling, hub_modules) =
        compute_coupling_and_hubs(&graph.imports_by_file, &graph.importers_by_file, &all_files);
    let cross_imports = detect_cross_imports(&graph.imports_by_file);
    let cycles = find_cycles(&graph.imports_by_file);
    let deep_chains = find_longest_chains(&graph.imports_by_file);
    let layer_flows = compute_layer_flows(&graph.imports_by_file);
    let orphans = find_orphan_modules(conn, &graph.importers_by_file).await?;
    let symbol_hotspots = find_symbol_hotspots(conn).await?;

    let total_modules = all_files.len();
    let total_imports = graph.raw_import_count;
    let resolved_imports: usize = graph.imports_by_file.values().map(|s| s.len()).sum();

    let stmt = conn.prepare("SELECT COUNT(*) FROM symbols").await?;
    let mut rows = stmt.query(()).await?;
    let total_symbols: i64 = if let Some(row) = rows.next().await? {
        row.get(0)?
    } else {
        0
    };

    Ok(ArchitectureReport {
        cycles,
        cross_imports,
        hub_modules,
        deep_chains,
        layer_flows,
        coupling_hotspots: coupling,
        orphan_modules: orphans,
        symbol_hotspots,
        total_modules,
        total_symbols: total_symbols as usize,
        total_imports,
        resolved_imports,
    })
}

/// Find cycles in the import graph using DFS
fn find_cycles(graph: &HashMap<String, HashSet<String>>) -> Vec<Cycle> {
    let mut cycles = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut rec_stack: HashSet<String> = HashSet::new();
    let mut path: Vec<String> = Vec::new();

    for node in graph.keys() {
        if !visited.contains(node) {
            find_cycles_dfs(
                node,
                graph,
                &mut visited,
                &mut rec_stack,
                &mut path,
                &mut cycles,
            );
        }
    }

    // Deduplicate cycles (same cycle can be found starting from different nodes)
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

    unique_cycles.truncate(10); // Limit to 10 cycles
    unique_cycles
}

fn find_cycles_dfs(
    node: &str,
    graph: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
    cycles: &mut Vec<Cycle>,
) {
    visited.insert(node.to_string());
    rec_stack.insert(node.to_string());
    path.push(node.to_string());

    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                find_cycles_dfs(neighbor, graph, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(neighbor) {
                // Found a cycle - extract it from path
                if let Some(pos) = path.iter().position(|x| x == neighbor) {
                    let cycle_path: Vec<String> = path[pos..].to_vec();
                    if cycle_path.len() > 1 {
                        cycles.push(Cycle {
                            modules: cycle_path,
                        });
                    }
                }
            }
        }
    }

    path.pop();
    rec_stack.remove(node);
}

/// Find the longest import chains (dependency paths) in the graph.
/// Uses DFS to find the longest path from each node, avoiding cycles.
fn find_longest_chains(graph: &HashMap<String, HashSet<String>>) -> Vec<ImportChain> {
    let mut longest_paths: Vec<ImportChain> = Vec::new();
    let mut memo: HashMap<String, Vec<String>> = HashMap::new();

    // Find longest path starting from each node
    for start in graph.keys() {
        let mut visited: HashSet<String> = HashSet::new();
        let path = longest_path_from(start, graph, &mut visited, &mut memo);
        if path.len() > 3 {
            // Only report chains with depth > 2 (3+ nodes)
            longest_paths.push(ImportChain {
                depth: path.len() - 1,
                modules: path,
            });
        }
    }

    // Sort by depth descending, deduplicate by first node, take top 5
    longest_paths.sort_by(|a, b| b.depth.cmp(&a.depth));

    // Deduplicate - if a shorter chain is a suffix of a longer one, skip it
    let mut unique_chains: Vec<ImportChain> = Vec::new();
    for chain in longest_paths {
        let dominated = unique_chains.iter().any(|existing| {
            // Check if chain is a suffix of existing
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

/// Extract the layer (top-level directory) from a file path.
/// Returns the first significant directory component.
fn extract_layer(path: &str) -> String {
    // Skip common prefixes like "crates/normalize/" to get to meaningful layer
    let path = path
        .strip_prefix("crates/normalize/src/")
        .or_else(|| path.strip_prefix("crates/normalize-"))
        .or_else(|| path.strip_prefix("src/"))
        .unwrap_or(path);

    // Get first directory component
    if let Some(pos) = path.find('/') {
        path[..pos].to_string()
    } else {
        // File in root - use filename without extension as "layer"
        path.split('.').next().unwrap_or("root").to_string()
    }
}

/// Compute import flows between directory layers.
fn compute_layer_flows(graph: &HashMap<String, HashSet<String>>) -> Vec<LayerFlow> {
    let mut flow_counts: HashMap<(String, String), usize> = HashMap::new();

    for (from_file, to_files) in graph {
        let from_layer = extract_layer(from_file);
        for to_file in to_files {
            let to_layer = extract_layer(to_file);
            // Only count cross-layer imports
            if from_layer != to_layer {
                *flow_counts
                    .entry((from_layer.clone(), to_layer.clone()))
                    .or_insert(0) += 1;
            }
        }
    }

    // Convert to vec and sort by count descending
    let mut flows: Vec<LayerFlow> = flow_counts
        .into_iter()
        .map(|((from, to), count)| LayerFlow {
            from_layer: from,
            to_layer: to,
            count,
        })
        .collect();

    flows.sort_by(|a, b| b.count.cmp(&a.count));
    flows.truncate(15); // Top 15 flows
    flows
}
