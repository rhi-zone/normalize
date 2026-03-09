//! Graph-theoretic metrics on the dependency graph.
//!
//! Operates on either the module graph (file→file via imports) or the symbol
//! graph (function→function via calls). Use `--on modules` (default) or
//! `--on symbols` to choose.
//!
//! Computes structural properties: strongly connected components (Tarjan's),
//! diamond dependencies, bridge edges, transitive (redundant) edges,
//! deep chains, and overall graph density.
//!
//! Pure graph algorithms and output formatting live in `normalize-graph`.
//! This module handles graph construction from the index and CLI wiring.

use crate::index::FileIndex;
use normalize_graph::{BlastRadius, DependentEntry, analyze_graph_data, find_dependents};
use normalize_languages::is_test_path;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

pub use normalize_graph::{DependentsReport, GraphReport, GraphTarget};

// ---------------------------------------------------------------------------
// Graph construction
// ---------------------------------------------------------------------------

/// Build the symbol-level call graph: nodes are "file:symbol", edges are calls.
async fn build_call_graph(
    idx: &FileIndex,
) -> Result<HashMap<String, HashSet<String>>, libsql::Error> {
    let conn = idx.connection();
    let stmt = conn
        .prepare("SELECT caller_file, caller_symbol, callee_name FROM calls")
        .await?;
    let mut rows = stmt.query(()).await?;

    // First pass: collect raw edges and build a callee_name → set of "file:symbol" lookup
    let mut raw_edges: Vec<(String, String, String)> = Vec::new();
    while let Some(row) = rows.next().await? {
        let caller_file: String = row.get(0)?;
        let caller_symbol: String = row.get(1)?;
        let callee_name: String = row.get(2)?;
        raw_edges.push((caller_file, caller_symbol, callee_name));
    }

    // Build callee resolution: name → [(file, symbol)]
    let stmt = conn.prepare("SELECT file, name FROM symbols").await?;
    let mut rows = stmt.query(()).await?;
    let mut symbol_locations: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(row) = rows.next().await? {
        let file: String = row.get(0)?;
        let name: String = row.get(1)?;
        let key = format!("{}:{}", file, name);
        symbol_locations.entry(name).or_default().push(key);
    }

    // Build adjacency list
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
    for (caller_file, caller_symbol, callee_name) in &raw_edges {
        let caller_key = format!("{}:{}", caller_file, caller_symbol);
        if let Some(targets) = symbol_locations.get(callee_name) {
            for target in targets {
                if target != &caller_key {
                    graph
                        .entry(caller_key.clone())
                        .or_default()
                        .insert(target.clone());
                }
            }
        }
    }

    Ok(graph)
}

/// Build the type-level dependency graph: nodes are type names, edges are type references.
async fn build_type_graph(
    idx: &FileIndex,
) -> Result<HashMap<String, HashSet<String>>, libsql::Error> {
    let conn = idx.connection();
    let stmt = conn
        .prepare("SELECT source_symbol, target_type FROM type_refs")
        .await?;
    let mut rows = stmt.query(()).await?;

    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
    while let Some(row) = rows.next().await? {
        let source: String = row.get(0)?;
        let target: String = row.get(1)?;
        if source != target {
            graph.entry(source).or_default().insert(target);
        }
    }

    Ok(graph)
}

// ---------------------------------------------------------------------------
// Main analysis
// ---------------------------------------------------------------------------

/// Analyze graph-theoretic properties of the dependency graph.
pub async fn analyze_graph(
    idx: &FileIndex,
    limit: usize,
    target: GraphTarget,
) -> Result<GraphReport, libsql::Error> {
    let adj = match target {
        GraphTarget::Modules => {
            use super::architecture::build_import_graph;
            let graph = build_import_graph(idx).await?;
            graph.imports_by_file
        }
        GraphTarget::Symbols => build_call_graph(idx).await?,
        GraphTarget::Types => build_type_graph(idx).await?,
    };

    Ok(analyze_graph_data(&adj, target, limit))
}

/// Find all modules/symbols that (transitively) depend on a given file.
///
/// For modules: returns structured output with depth, test coverage, fan-in,
/// blast radius statistics, and untested impact paths.
/// For symbols/types: returns a flat alphabetical list.
pub async fn analyze_dependents(
    idx: &FileIndex,
    file: &str,
    target: GraphTarget,
) -> Result<DependentsReport, libsql::Error> {
    match target {
        GraphTarget::Modules => analyze_module_dependents(idx, file).await,
        GraphTarget::Symbols => {
            let adj = build_call_graph(idx).await?;
            let dependents = find_dependents(&adj, file);
            Ok(DependentsReport {
                target: file.to_string(),
                graph_target: target,
                dependents,
                direct: Vec::new(),
                transitive: Vec::new(),
                blast_radius: None,
                untested_paths: Vec::new(),
            })
        }
        GraphTarget::Types => {
            let adj = build_type_graph(idx).await?;
            let dependents = find_dependents(&adj, file);
            Ok(DependentsReport {
                target: file.to_string(),
                graph_target: target,
                dependents,
                direct: Vec::new(),
                transitive: Vec::new(),
                blast_radius: None,
                untested_paths: Vec::new(),
            })
        }
    }
}

/// Blast-radius analysis for a single file in the modules (import) graph.
async fn analyze_module_dependents(
    idx: &FileIndex,
    file: &str,
) -> Result<DependentsReport, libsql::Error> {
    use super::architecture::build_import_graph;

    let graph = build_import_graph(idx).await?;

    // Compute fan-in per file
    let fan_in: HashMap<&str, usize> = graph
        .importers_by_file
        .iter()
        .map(|(f, importers)| (f.as_str(), importers.len()))
        .collect();

    // BFS through reverse edges (importers_by_file)
    let mut visited: HashMap<String, usize> = HashMap::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    if let Some(importers) = graph.importers_by_file.get(file) {
        for importer in importers {
            if !visited.contains_key(importer) {
                visited.insert(importer.clone(), 1);
                queue.push_back((importer.clone(), 1));
            }
        }
    }

    while let Some((f, depth)) = queue.pop_front() {
        if let Some(importers) = graph.importers_by_file.get(&f) {
            for importer in importers {
                if !visited.contains_key(importer) && importer != file {
                    visited.insert(importer.clone(), depth + 1);
                    queue.push_back((importer.clone(), depth + 1));
                }
            }
        }
    }

    // Build entries
    let mut direct: Vec<DependentEntry> = Vec::new();
    let mut transitive: Vec<DependentEntry> = Vec::new();

    for (f, depth) in &visited {
        let entry = DependentEntry {
            file: f.clone(),
            depth: *depth,
            has_tests: is_test_path(Path::new(f)),
            fan_in: fan_in.get(f.as_str()).copied().unwrap_or(0),
        };
        if *depth == 1 {
            direct.push(entry);
        } else {
            transitive.push(entry);
        }
    }

    direct.sort_by(|a, b| a.depth.cmp(&b.depth).then(b.fan_in.cmp(&a.fan_in)));
    transitive.sort_by(|a, b| a.depth.cmp(&b.depth).then(b.fan_in.cmp(&a.fan_in)));

    let untested_paths = build_untested_paths(&visited, &graph.importers_by_file);

    let max_depth = visited.values().copied().max().unwrap_or(0);
    let untested_count = direct
        .iter()
        .chain(transitive.iter())
        .filter(|e| !e.has_tests)
        .count();

    let blast_radius = BlastRadius {
        direct_count: direct.len(),
        transitive_count: transitive.len(),
        untested_count,
        max_depth,
    };

    Ok(DependentsReport {
        target: file.to_string(),
        graph_target: GraphTarget::Modules,
        direct,
        transitive,
        blast_radius: Some(blast_radius),
        untested_paths,
        dependents: Vec::new(),
    })
}

/// Build untested impact path descriptions.
fn build_untested_paths(
    visited: &HashMap<String, usize>,
    importers_by_file: &HashMap<String, std::collections::HashSet<String>>,
) -> Vec<String> {
    let mut untested: Vec<(&String, usize)> = visited
        .iter()
        .filter(|(f, _)| !is_test_path(Path::new(f)))
        .map(|(f, d)| (f, *d))
        .collect();
    untested.sort_by_key(|(_, d)| *d);

    let mut paths = Vec::new();
    for (f, depth) in &untested {
        if *depth != 1 {
            continue;
        }
        let mut chain = vec![format!("{} (depth 1)", f)];
        if let Some(next_importers) = importers_by_file.get(*f) {
            for next in next_importers {
                if let Some(nd) = visited.get(next)
                    && !is_test_path(Path::new(next))
                    && *nd > 1
                {
                    chain.push(format!("{} (depth {})", next, nd));
                }
            }
        }
        if chain.len() > 1 {
            paths.push(chain.join(" → "));
        } else {
            paths.push(chain[0].clone());
        }
    }

    paths
}
