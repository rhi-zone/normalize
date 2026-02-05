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

/// Full architecture analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ArchitectureReport {
    pub cycles: Vec<Cycle>,
    pub coupling_hotspots: Vec<ModuleCoupling>,
    pub cross_imports: Vec<CrossImport>,
    pub orphan_modules: Vec<OrphanModule>,
    pub symbol_hotspots: Vec<SymbolMetrics>,
    pub total_modules: usize,
    pub total_symbols: usize,
    pub total_imports: usize,
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
        lines.push(format!("  Import relationships: {}", self.total_imports));
        lines.push(format!("  Circular dependencies: {}", self.cycles.len()));
        lines.push(format!("  Cross-imports: {}", self.cross_imports.len()));
        lines.push(format!("  Orphan modules: {}", self.orphan_modules.len()));

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
    let idx = match rt.block_on(FileIndex::open_if_enabled(root)) {
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

async fn analyze_architecture(idx: &FileIndex) -> Result<ArchitectureReport, libsql::Error> {
    // Build import graph: file -> set of imported modules
    let mut imports_by_file: HashMap<String, HashSet<String>> = HashMap::new();
    let mut importers_by_module: HashMap<String, HashSet<String>> = HashMap::new();

    // Query all imports
    let conn = idx.connection();
    let stmt = conn
        .prepare("SELECT file, module FROM imports WHERE module IS NOT NULL")
        .await?;
    let mut rows = stmt.query(()).await?;

    while let Some(row) = rows.next().await? {
        let file: String = row.get(0)?;
        let module: String = row.get(1)?;

        imports_by_file
            .entry(file.clone())
            .or_default()
            .insert(module.clone());
        importers_by_module.entry(module).or_default().insert(file);
    }

    // Get all source files (files with symbols indexed, programming languages only)
    let mut all_files: HashSet<String> = HashSet::new();
    let stmt = conn.prepare("SELECT DISTINCT file FROM symbols").await?;
    let mut rows = stmt.query(()).await?;
    while let Some(row) = rows.next().await? {
        let path: String = row.get(0)?;
        if is_programming_language(Path::new(&path)) {
            all_files.insert(path);
        }
    }

    // Calculate module coupling
    let mut coupling: Vec<ModuleCoupling> = Vec::new();
    for file in &all_files {
        let fan_out = imports_by_file.get(file).map(|s| s.len()).unwrap_or(0);
        let fan_in = importers_by_module.get(file).map(|s| s.len()).unwrap_or(0);
        let total = fan_in + fan_out;
        let instability = if total > 0 {
            fan_out as f64 / total as f64
        } else {
            0.5 // No connections = neutral
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

    // Sort by fan_in (most depended on first)
    coupling.sort_by(|a, b| b.fan_in.cmp(&a.fan_in));
    coupling.truncate(15);

    // Find cross-imports (A imports B AND B imports A)
    let mut cross_imports: Vec<CrossImport> = Vec::new();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();

    for (file_a, imports_a) in &imports_by_file {
        for module_b in imports_a {
            // Check if module_b imports file_a
            if let Some(imports_b) = imports_by_file.get(module_b)
                && imports_b.contains(file_a)
            {
                let pair = if file_a < module_b {
                    (file_a.clone(), module_b.clone())
                } else {
                    (module_b.clone(), file_a.clone())
                };
                if !seen_pairs.contains(&pair) {
                    seen_pairs.insert(pair);
                    // Count imports in each direction
                    let a_to_b = imports_a.iter().filter(|m| *m == module_b).count();
                    let b_to_a = imports_b.iter().filter(|m| *m == file_a).count();
                    cross_imports.push(CrossImport {
                        module_a: file_a.clone(),
                        module_b: module_b.clone(),
                        a_to_b,
                        b_to_a,
                    });
                }
            }
        }
    }

    // Detect cycles using DFS
    let cycles = find_cycles(&imports_by_file);

    // Find orphan modules (files with symbols but never imported)
    let mut orphans: Vec<OrphanModule> = Vec::new();
    let stmt = conn
        .prepare("SELECT file, COUNT(*) FROM symbols GROUP BY file")
        .await?;
    let mut rows = stmt.query(()).await?;
    while let Some(row) = rows.next().await? {
        let file: String = row.get(0)?;
        let symbol_count: i64 = row.get(1)?;

        // Only consider programming languages (not config/data formats)
        if !is_programming_language(Path::new(&file)) {
            continue;
        }

        // Is this file imported by anyone?
        let is_imported = importers_by_module.contains_key(&file);

        // Skip main/entry files and test files
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

    // Symbol hotspots (most called functions)
    // Filter out generic/common methods that aren't interesting
    let generic_methods: HashSet<&str> = [
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
    ]
    .into_iter()
    .collect();

    let mut symbol_callers: HashMap<String, (String, String, usize)> = HashMap::new();
    let stmt = conn
        .prepare("SELECT callee_name, COUNT(*) as cnt FROM calls GROUP BY callee_name ORDER BY cnt DESC LIMIT 100")
        .await?;
    let mut rows = stmt.query(()).await?;
    while let Some(row) = rows.next().await? {
        let name: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        // Skip generic methods
        if !generic_methods.contains(name.as_str()) {
            symbol_callers.insert(name, (String::new(), String::new(), count as usize));
        }
    }

    // Resolve symbol details
    let mut symbol_hotspots: Vec<SymbolMetrics> = Vec::new();
    for (name, (_, _, callers)) in &symbol_callers {
        // Find the symbol definition
        let stmt = conn
            .prepare("SELECT file, kind FROM symbols WHERE name = ? LIMIT 1")
            .await?;
        let mut rows = stmt.query([name.as_str()]).await?;
        if let Some(row) = rows.next().await? {
            let file: String = row.get(0)?;
            let kind: String = row.get(1)?;
            if *callers > 3 {
                // Only show symbols called more than 3 times
                symbol_hotspots.push(SymbolMetrics {
                    file,
                    name: name.clone(),
                    kind,
                    callers: *callers,
                });
            }
        }
    }
    symbol_hotspots.sort_by(|a, b| b.callers.cmp(&a.callers));
    symbol_hotspots.truncate(10);

    // Count totals
    let total_modules = all_files.len();
    let total_imports: usize = imports_by_file.values().map(|s| s.len()).sum();

    let stmt = conn.prepare("SELECT COUNT(*) FROM symbols").await?;
    let mut rows = stmt.query(()).await?;
    let total_symbols: i64 = if let Some(row) = rows.next().await? {
        row.get(0)?
    } else {
        0
    };

    Ok(ArchitectureReport {
        cycles,
        coupling_hotspots: coupling,
        cross_imports,
        orphan_modules: orphans,
        symbol_hotspots,
        total_modules,
        total_symbols: total_symbols as usize,
        total_imports,
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
