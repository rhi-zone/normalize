//! Graph-theoretic metrics on the dependency graph — the `graph` CLI verb.
//!
//! Operates on either the module graph (file→file via imports) or the symbol
//! graph (function→function via calls). Use `--on modules` (default) or
//! `--on symbols` to choose.
//!
//! Computes structural properties: strongly connected components (Tarjan's),
//! diamond dependencies, bridge edges, transitive (redundant) edges,
//! deep chains, and overall graph density.
//!
//! Pure graph algorithms live in the crate root (`crate::`). This module handles
//! graph construction from the index, report assembly, and output formatting —
//! the normalize-flavored presentation layer over the crate's generic primitives.

use crate::{
    BlastRadius, BridgeEdge, DependentEntry, Diamond, ImportChain, Scc, TransitiveEdge, all_nodes,
    count_transitive_edges, edge_count, find_bridges, find_dead_nodes, find_dependents,
    find_diamonds, find_longest_chains, find_sccs, find_transitive_edges, tarjan_sccs,
    weakly_connected_components,
};
use normalize_index::{FileIndex, build_import_graph};
use normalize_languages::is_test_path;
use normalize_output::OutputFormatter;
use nu_ansi_term::Color;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

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
            let graph = build_import_graph(idx).await?;
            graph.imports_by_file
        }
        GraphTarget::Symbols => build_call_graph(idx).await?,
        GraphTarget::Types => build_type_graph(idx).await?,
    };

    Ok(assemble_graph_report(&adj, target, limit))
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

// ---------------------------------------------------------------------------
// Report types (normalize-flavored presentation over the pure algorithms)
// ---------------------------------------------------------------------------

/// What the graph nodes represent.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum GraphTarget {
    /// Nodes are files, edges are imports
    Modules,
    /// Nodes are functions (file:symbol), edges are calls
    Symbols,
    /// Nodes are types, edges are type references (fields, params, inheritance, etc.)
    Types,
}

impl std::str::FromStr for GraphTarget {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "modules" => Ok(Self::Modules),
            "symbols" => Ok(Self::Symbols),
            "types" => Ok(Self::Types),
            _ => Err(format!(
                "unknown graph target '{}', expected 'modules', 'symbols', or 'types'",
                s
            )),
        }
    }
}

impl std::fmt::Display for GraphTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Modules => write!(f, "modules"),
            Self::Symbols => write!(f, "symbols"),
            Self::Types => write!(f, "types"),
        }
    }
}

/// Overall graph statistics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GraphStats {
    pub nodes: usize,
    pub edges: usize,
    /// Edge density: edges / (nodes × (nodes − 1)); 0.0 for graphs with fewer than 2 nodes.
    pub density: f64,
    pub weakly_connected_components: usize,
    pub largest_component_size: usize,
    pub scc_count: usize,
    /// Number of strongly connected components with more than one node (actual circular-dependency clusters).
    pub nontrivial_scc_count: usize,
    pub diamond_count: usize,
    /// Number of bridge edges whose removal would disconnect the graph.
    pub bridge_count: usize,
    /// Number of redundant transitive edges (A→C where A→B→C already exists).
    pub transitive_edge_count: usize,
    /// Depth (edge count) of the longest import chain.
    pub max_chain_depth: usize,
    /// Total number of import chains at or exceeding the depth threshold.
    pub chain_count: usize,
    /// Number of nodes with no inbound edges (unreachable or potentially dead code).
    pub dead_node_count: usize,
}

/// Full graph analysis report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GraphReport {
    pub target: GraphTarget,
    pub stats: GraphStats,
    pub sccs: Vec<Scc>,
    pub diamonds: Vec<Diamond>,
    pub bridges: Vec<BridgeEdge>,
    pub longest_chains: Vec<ImportChain>,
    pub transitive_edges: Vec<TransitiveEdge>,
    /// Nodes with no inbound edges (files/symbols that nothing imports/calls).
    /// Sorted alphabetically. Does not include fully isolated nodes (no edges at all).
    pub dead_nodes: Vec<String>,
}

/// Report for reverse dependency queries: what depends on a given file/symbol.
///
/// For `--on modules` (default): structured output with depth, test coverage,
/// fan-in, and blast radius statistics.
/// For `--on symbols` / `--on types`: flat alphabetical list of dependents.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DependentsReport {
    /// The file or symbol being queried.
    pub target: String,
    /// Graph node kind used for the query.
    pub graph_target: GraphTarget,
    /// Direct dependents (depth = 1) — populated for modules graph.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub direct: Vec<DependentEntry>,
    /// Transitive dependents (depth > 1) — populated for modules graph.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub transitive: Vec<DependentEntry>,
    /// Blast radius summary — populated for modules graph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blast_radius: Option<BlastRadius>,
    /// Untested impact paths — populated for modules graph.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub untested_paths: Vec<String>,
    /// Flat dependent list — populated for symbols/types graph.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependents: Vec<String>,
}

/// Report for `normalize graph import-path`.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ImportPathReport {
    /// The source file (root-relative).
    pub from: String,
    /// The target file (root-relative).
    pub to: String,
    /// Found import paths.  Empty when no path exists.
    /// Each inner vec is one path: [from, hop1, ..., to].
    pub paths: Vec<Vec<String>>,
    /// True when `--all` was requested (multiple paths may be present).
    pub all_paths: bool,
}

// ---------------------------------------------------------------------------
// Report assembly (calls the pure algorithms from the crate root)
// ---------------------------------------------------------------------------

/// Assemble the full graph analysis report from an abstract dependency graph.
///
/// Takes an adjacency list (`HashMap<String, HashSet<String>>`) and a limit
/// for the number of items to return in each section. Pass `0` or `usize::MAX`
/// for no limit — `0` is treated as "unlimited" so callers do not accidentally
/// truncate all results to empty Vecs while stats counts still reflect the
/// full data (which would produce misleading reports). The `target` parameter
/// is recorded in the report for display.
fn assemble_graph_report(
    imports: &HashMap<String, HashSet<String>>,
    target: GraphTarget,
    limit: usize,
) -> GraphReport {
    // Treat 0 as "no limit" — callers should pass usize::MAX explicitly when
    // they want unlimited, but 0 is a common default and should not silently
    // truncate every result Vec to empty.
    let limit = if limit == 0 { usize::MAX } else { limit };
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

    let mut longest_chains = find_longest_chains(
        imports,
        if limit == usize::MAX {
            usize::MAX
        } else {
            limit
        },
    );
    let max_chain_depth = longest_chains.first().map(|c| c.depth).unwrap_or(0);
    let chain_count = longest_chains.len();

    let transitive_edge_count = count_transitive_edges(imports);
    let mut transitive_edges = find_transitive_edges(
        imports,
        if limit == usize::MAX {
            usize::MAX
        } else {
            limit
        },
    );

    let mut dead_nodes = find_dead_nodes(imports);
    let dead_node_count = dead_nodes.len();

    // Apply limits
    sccs.truncate(limit);
    diamonds.truncate(limit);
    longest_chains.truncate(limit);
    transitive_edges.truncate(limit);
    dead_nodes.truncate(limit);

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
        max_chain_depth,
        chain_count,
        dead_node_count,
    };

    GraphReport {
        target,
        stats,
        sccs,
        diamonds,
        bridges,
        longest_chains,
        transitive_edges,
        dead_nodes,
    }
}

// ---------------------------------------------------------------------------
// import-path query
// ---------------------------------------------------------------------------

/// Resolve a user-supplied path to a root-relative string for DB lookup.
///
/// Accepts either an absolute path or a path relative to `root`.
/// Strips the root prefix and normalizes separators.
fn resolve_db_path(input: &str, root: &Path) -> String {
    let p = PathBuf::from(input);
    let abs = if p.is_absolute() { p } else { root.join(p) };
    // Strip root prefix to get root-relative string
    abs.strip_prefix(root)
        .map(|r| r.to_string_lossy().into_owned())
        .unwrap_or_else(|_| input.to_string())
}

/// Find import path(s) between `from_file` and `to_file`.
pub async fn find_import_path_command(
    idx: &FileIndex,
    root: &Path,
    from_file: &str,
    to_file: &str,
    all_paths: bool,
    path_limit: usize,
    reverse: bool,
) -> Result<ImportPathReport, libsql::Error> {
    const MAX_DEPTH: usize = 10;

    let (from_raw, to_raw) = if reverse {
        (
            resolve_db_path(to_file, root),
            resolve_db_path(from_file, root),
        )
    } else {
        (
            resolve_db_path(from_file, root),
            resolve_db_path(to_file, root),
        )
    };

    let paths = idx
        .find_import_path(&from_raw, &to_raw, all_paths, path_limit, MAX_DEPTH)
        .await?;

    let (report_from, report_to) = if reverse {
        (to_raw, from_raw)
    } else {
        (from_raw, to_raw)
    };

    Ok(ImportPathReport {
        from: report_from,
        to: report_to,
        paths,
        all_paths,
    })
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        // Use char_indices to find a safe character boundary, avoiding a byte-index
        // slice into a multi-byte UTF-8 sequence which would panic.
        let suffix = path
            .char_indices()
            .rev()
            .find(|(i, _)| path.len() - i <= max_len - 3)
            .map(|(i, _)| &path[i..])
            .unwrap_or(path);
        format!("...{}", suffix)
    }
}

impl OutputFormatter for GraphReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();
        let s = &self.stats;

        let label = match self.target {
            GraphTarget::Modules => "Module graph",
            GraphTarget::Symbols => "Symbol graph",
            GraphTarget::Types => "Type graph",
        };
        out.push(format!(
            "# {} — {} nodes, {} edges, density {:.3}",
            label, s.nodes, s.edges, s.density
        ));
        out.push(format!(
            "  {} weakly connected components (largest: {})",
            s.weakly_connected_components, s.largest_component_size
        ));
        out.push(format!(
            "  {} circular-dependency clusters, {} diamonds, {} bridges, {} transitive edges",
            s.nontrivial_scc_count, s.diamond_count, s.bridge_count, s.transitive_edge_count
        ));
        if s.max_chain_depth > 0 {
            out.push(format!(
                "  max chain depth {}, {} deep chains (depth > 2)",
                s.max_chain_depth, s.chain_count
            ));
        }
        if s.dead_node_count > 0 {
            out.push(format!(
                "  {} unreferenced nodes (no inbound edges)",
                s.dead_node_count
            ));
        }
        out.push(String::new());

        if s.nodes == 0 {
            out.push("No data found. Run `normalize structure rebuild` first.".to_string());
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

        // Longest chains
        if !self.longest_chains.is_empty() {
            out.push(format!(
                "## Deep import chains ({}, max depth {})",
                self.longest_chains.len(),
                self.stats.max_chain_depth
            ));
            for chain in &self.longest_chains {
                let short_modules: Vec<String> =
                    chain.modules.iter().map(|m| truncate_path(m, 30)).collect();
                out.push(format!(
                    "  [depth {}] {}",
                    chain.depth,
                    short_modules.join(" → ")
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

        // Dead nodes (no inbound edges)
        if !self.dead_nodes.is_empty() {
            let label = match self.target {
                GraphTarget::Modules => "Unreferenced modules",
                GraphTarget::Symbols => "Uncalled symbols",
                GraphTarget::Types => "Unreferenced types",
            };
            out.push(format!(
                "## {} ({} nodes with no inbound edges)",
                label,
                self.dead_nodes.len()
            ));
            for node in &self.dead_nodes {
                out.push(format!("  {}", node));
            }
            out.push(String::new());
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut out = Vec::new();
        let s = &self.stats;

        let label = match self.target {
            GraphTarget::Modules => "Module graph",
            GraphTarget::Symbols => "Symbol graph",
            GraphTarget::Types => "Type graph",
        };
        out.push(format!(
            "\x1b[1;36m# {}\x1b[0m — \x1b[1m{}\x1b[0m nodes, \x1b[1m{}\x1b[0m edges, density \x1b[33m{:.3}\x1b[0m",
            label, s.nodes, s.edges, s.density
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
        if s.max_chain_depth > 0 {
            let depth_color = if s.max_chain_depth >= 5 {
                "\x1b[1;31m"
            } else if s.max_chain_depth >= 3 {
                "\x1b[33m"
            } else {
                "\x1b[32m"
            };
            out.push(format!(
                "  max chain depth {}{}\x1b[0m, {} deep chains (depth > 2)",
                depth_color, s.max_chain_depth, s.chain_count
            ));
        }
        if s.dead_node_count > 0 {
            out.push(format!(
                "  \x1b[2m{} unreferenced nodes (no inbound edges)\x1b[0m",
                s.dead_node_count
            ));
        }
        out.push(String::new());

        if s.nodes == 0 {
            out.push("No data found. Run `normalize structure rebuild` first.".to_string());
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

        // Longest chains
        if !self.longest_chains.is_empty() {
            out.push(format!(
                "\x1b[1m## Deep import chains ({}, max depth {})\x1b[0m",
                self.longest_chains.len(),
                self.stats.max_chain_depth
            ));
            for chain in &self.longest_chains {
                let short_modules: Vec<String> =
                    chain.modules.iter().map(|m| truncate_path(m, 30)).collect();
                let depth_color = if chain.depth >= 5 {
                    "\x1b[1;31m"
                } else if chain.depth >= 3 {
                    "\x1b[33m"
                } else {
                    "\x1b[32m"
                };
                out.push(format!(
                    "  {}[depth {}]\x1b[0m {}",
                    depth_color,
                    chain.depth,
                    short_modules.join(" \x1b[2m→\x1b[0m ")
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

        // Dead nodes (no inbound edges)
        if !self.dead_nodes.is_empty() {
            let label = match self.target {
                GraphTarget::Modules => "Unreferenced modules",
                GraphTarget::Symbols => "Uncalled symbols",
                GraphTarget::Types => "Unreferenced types",
            };
            out.push(format!(
                "\x1b[2m## {} ({} with no inbound edges)\x1b[0m",
                label,
                self.dead_nodes.len()
            ));
            for node in &self.dead_nodes {
                out.push(format!("  \x1b[2m{}\x1b[0m", node));
            }
            out.push(String::new());
        }

        out.join("\n")
    }
}

impl OutputFormatter for DependentsReport {
    fn format_text(&self) -> String {
        match self.graph_target {
            GraphTarget::Modules => self.format_modules_text(false),
            GraphTarget::Symbols | GraphTarget::Types => self.format_flat_text(false),
        }
    }

    fn format_pretty(&self) -> String {
        match self.graph_target {
            GraphTarget::Modules => self.format_modules_text(true),
            GraphTarget::Symbols | GraphTarget::Types => self.format_flat_text(true),
        }
    }
}

impl DependentsReport {
    fn format_modules_text(&self, pretty: bool) -> String {
        let mut lines = Vec::new();
        let total = self.direct.len() + self.transitive.len();

        if pretty {
            lines.push(
                Color::Cyan
                    .bold()
                    .paint(format!("# Dependents of {}", self.target))
                    .to_string(),
            );
        } else {
            lines.push(format!("# Dependents of {}", self.target));
        }
        lines.push(String::new());

        if let Some(ref br) = self.blast_radius {
            if pretty {
                lines.push(format!(
                    "{} files affected · {} direct · {} transitive · {} untested · max depth {}",
                    Color::Default.bold().paint(total.to_string()),
                    Color::Green.paint(br.direct_count.to_string()),
                    Color::Yellow.paint(br.transitive_count.to_string()),
                    Color::Red.paint(br.untested_count.to_string()),
                    br.max_depth
                ));
            } else {
                lines.push(format!(
                    "{} files affected · {} direct · {} transitive · {} untested · max depth {}",
                    total, br.direct_count, br.transitive_count, br.untested_count, br.max_depth
                ));
            }
        }

        if !self.direct.is_empty() {
            lines.push(String::new());
            if pretty {
                lines.push(
                    Color::Green
                        .bold()
                        .paint(format!("## Direct ({})", self.direct.len()))
                        .to_string(),
                );
            } else {
                lines.push(format!("## Direct ({})", self.direct.len()));
            }
            for e in &self.direct {
                let tested = if e.has_tests {
                    if pretty {
                        Color::Green.paint("tested").to_string()
                    } else {
                        "tested".to_string()
                    }
                } else if pretty {
                    Color::Red.bold().paint("UNTESTED").to_string()
                } else {
                    "UNTESTED".to_string()
                };
                lines.push(format!(
                    "  {:<40} depth {}  {}  fan-in {}",
                    e.file, e.depth, tested, e.fan_in
                ));
            }
        }

        if !self.transitive.is_empty() {
            lines.push(String::new());
            if pretty {
                lines.push(
                    Color::Yellow
                        .bold()
                        .paint(format!("## Transitive ({})", self.transitive.len()))
                        .to_string(),
                );
            } else {
                lines.push(format!("## Transitive ({})", self.transitive.len()));
            }
            for e in &self.transitive {
                let tested = if e.has_tests {
                    if pretty {
                        Color::Green.paint("tested").to_string()
                    } else {
                        "tested".to_string()
                    }
                } else if pretty {
                    Color::Red.bold().paint("UNTESTED").to_string()
                } else {
                    "UNTESTED".to_string()
                };
                lines.push(format!(
                    "  {:<40} depth {}  {}  fan-in {}",
                    e.file, e.depth, tested, e.fan_in
                ));
            }
        }

        if !self.untested_paths.is_empty() {
            lines.push(String::new());
            if pretty {
                lines.push(
                    Color::Red
                        .bold()
                        .paint(format!(
                            "## Untested Impact Paths ({})",
                            self.untested_paths.len()
                        ))
                        .to_string(),
                );
            } else {
                lines.push(format!(
                    "## Untested Impact Paths ({})",
                    self.untested_paths.len()
                ));
            }
            for p in &self.untested_paths {
                lines.push(format!("  {}", p));
            }
        }

        lines.join("\n")
    }

    fn format_flat_text(&self, pretty: bool) -> String {
        let kind = match self.graph_target {
            GraphTarget::Modules => "modules",
            GraphTarget::Symbols => "symbols",
            GraphTarget::Types => "types",
        };
        let mut out = Vec::new();
        if pretty {
            out.push(format!(
                "{} {} {}",
                Color::Cyan.bold().paint("# Dependents of"),
                Color::Default.bold().paint(&self.target),
                Color::Default.dimmed().paint(format!(
                    "({} {} depend on it)",
                    self.dependents.len(),
                    kind
                )),
            ));
            for dep in &self.dependents {
                out.push(format!("  {}", Color::White.paint(dep.as_str())));
            }
        } else {
            out.push(format!(
                "# Dependents of {} ({} {} depend on it)",
                self.target,
                self.dependents.len(),
                kind
            ));
            for dep in &self.dependents {
                out.push(format!("  {}", dep));
            }
        }
        out.join("\n")
    }
}

impl OutputFormatter for ImportPathReport {
    fn format_text(&self) -> String {
        if self.from == self.to {
            return "Same file".to_string();
        }
        if self.paths.is_empty() {
            return format!("No import path found between {} and {}", self.from, self.to);
        }
        let mut lines = Vec::new();
        for (i, path) in self.paths.iter().enumerate() {
            if self.all_paths && self.paths.len() > 1 {
                lines.push(format!("Path {}:", i + 1));
                lines.push(format!("  {}", path.join(" → ")));
            } else {
                lines.push(path.join(" → "));
            }
        }
        lines.join("\n")
    }
}
