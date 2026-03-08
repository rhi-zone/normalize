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
use normalize_graph::{analyze_graph_data, find_dependents};
use std::collections::{HashMap, HashSet};
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

/// CLI entry point (sync wrapper).
pub fn analyze_graph_sync(
    root: &Path,
    limit: usize,
    target: GraphTarget,
) -> Result<GraphReport, String> {
    crate::runtime::block_on(async {
        let idx = crate::index::ensure_ready(root).await?;
        analyze_graph(&idx, limit, target)
            .await
            .map_err(|e| format!("Graph analysis failed: {}", e))
    })
}

/// Find all modules/symbols that (transitively) depend on a given file.
pub async fn analyze_dependents(
    idx: &FileIndex,
    file: &str,
    target: GraphTarget,
) -> Result<DependentsReport, libsql::Error> {
    let adj = match target {
        GraphTarget::Modules => {
            use super::architecture::build_import_graph;
            let graph = build_import_graph(idx).await?;
            graph.imports_by_file
        }
        GraphTarget::Symbols => build_call_graph(idx).await?,
        GraphTarget::Types => build_type_graph(idx).await?,
    };

    let dependents = find_dependents(&adj, file);
    Ok(DependentsReport {
        target: file.to_string(),
        graph_target: target,
        dependents,
    })
}

/// CLI entry point for dependents query (sync wrapper).
pub fn analyze_dependents_sync(
    root: &Path,
    file: &str,
    target: GraphTarget,
) -> Result<DependentsReport, String> {
    crate::runtime::block_on(async {
        let idx = crate::index::ensure_ready(root).await?;
        analyze_dependents(&idx, file, target)
            .await
            .map_err(|e| format!("Dependents query failed: {}", e))
    })
}
