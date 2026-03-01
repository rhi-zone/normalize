use normalize_languages::support_for_path;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::analyze::complexity::ComplexityAnalyzer;
use crate::commands::analyze::test_ratio::module_key;
use crate::output::OutputFormatter;

/// Per-function call-complexity entry.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FunctionCallComplexity {
    pub file: String,
    pub symbol: String,
    pub local_cc: usize,
    /// Sum of CC for all reachable functions via BFS.
    pub reachable_cc: usize,
    /// Max CC on any path to a leaf.
    pub critical_path_cc: usize,
    /// reachable_cc / local_cc — high = thin dispatcher, low = self-contained
    pub amplification: f64,
    /// Number of distinct functions reachable.
    pub reachable_count: usize,
}

/// Per-module aggregated call complexity.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ModuleCallComplexity {
    pub module: String,
    pub avg_amplification: f64,
    pub max_reachable_cc: usize,
    pub total_local_cc: usize,
    pub function_count: usize,
}

/// Report returned by `analyze call-complexity`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CallComplexityReport {
    pub root: String,
    pub index_available: bool,
    pub total_functions: usize,
    /// Percentage of call edges we could not resolve to a known function.
    pub unresolved_callees_pct: f64,
    /// Highest amplification ratio — thin dispatchers into complex code.
    pub top_amplified: Vec<FunctionCallComplexity>,
    /// Highest absolute reachable CC — deep complexity sinks.
    pub top_reachable: Vec<FunctionCallComplexity>,
    pub modules: Vec<ModuleCallComplexity>,
}

impl OutputFormatter for CallComplexityReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();
        out.push("# Call-Complexity Analysis".to_string());
        out.push(String::new());
        out.push(format!("Root:               {}", self.root));
        out.push(format!("Index available:    {}", self.index_available));
        out.push(format!("Functions analyzed: {}", self.total_functions));
        out.push(format!(
            "Unresolved callees: {:.1}%",
            self.unresolved_callees_pct
        ));
        out.push(String::new());

        if !self.top_amplified.is_empty() {
            out.push("## Top Amplified (dispatcher → complex territory)".to_string());
            out.push(String::new());
            out.push(format!(
                "  {:<8}  {:<8}  {:<8}  {:<8}  symbol",
                "amplif", "local", "reach", "reach#"
            ));
            for f in &self.top_amplified {
                out.push(format!(
                    "  {:>8.1}x {:>8}  {:>8}  {:>8}  {}:{}",
                    f.amplification,
                    f.local_cc,
                    f.reachable_cc,
                    f.reachable_count,
                    f.file,
                    f.symbol
                ));
            }
            out.push(String::new());
        }

        if !self.top_reachable.is_empty() {
            out.push("## Highest Reachable CC (deepest complexity sinks)".to_string());
            out.push(String::new());
            out.push(format!(
                "  {:<8}  {:<8}  {:<8}  symbol",
                "reach", "local", "reach#"
            ));
            for f in &self.top_reachable {
                out.push(format!(
                    "  {:>8}  {:>8}  {:>8}  {}:{}",
                    f.reachable_cc, f.local_cc, f.reachable_count, f.file, f.symbol
                ));
            }
            out.push(String::new());
        }

        if !self.modules.is_empty() {
            out.push("## Modules".to_string());
            out.push(String::new());
            let w = self
                .modules
                .iter()
                .map(|m| m.module.len())
                .max()
                .unwrap_or(20);
            out.push(format!(
                "  {:<w$}  {:>5}  {:>8}  {:>10}  {:>8}",
                "module",
                "fns",
                "avg_amp",
                "max_reach",
                "local_cc",
                w = w
            ));
            out.push(format!(
                "  {:<w$}  {:>5}  {:>8}  {:>10}  {:>8}",
                "-".repeat(w),
                "-----",
                "--------",
                "----------",
                "--------",
                w = w
            ));
            for m in &self.modules {
                out.push(format!(
                    "  {:<w$}  {:>5}  {:>8.1}x {:>10}  {:>8}",
                    m.module,
                    m.function_count,
                    m.avg_amplification,
                    m.max_reachable_cc,
                    m.total_local_cc,
                    w = w
                ));
            }
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;
        let mut out = Vec::new();
        out.push(
            Color::Cyan
                .bold()
                .paint("# Call-Complexity Analysis")
                .to_string(),
        );
        out.push(String::new());
        out.push(format!("Root:               {}", self.root));
        out.push(format!("Index available:    {}", self.index_available));
        out.push(format!("Functions analyzed: {}", self.total_functions));
        out.push(format!(
            "Unresolved callees: {:.1}%",
            self.unresolved_callees_pct
        ));
        out.push(String::new());

        if !self.top_amplified.is_empty() {
            out.push(
                Color::Yellow
                    .bold()
                    .paint("## Top Amplified (dispatcher → complex territory)")
                    .to_string(),
            );
            out.push(String::new());
            out.push(format!(
                "  {:<8}  {:<8}  {:<8}  {:<8}  symbol",
                Color::White.bold().paint("amplif"),
                Color::White.bold().paint("local"),
                Color::White.bold().paint("reach"),
                Color::White.bold().paint("reach#")
            ));
            for f in &self.top_amplified {
                let color = if f.amplification > 20.0 {
                    Color::Red
                } else if f.amplification > 10.0 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                out.push(format!(
                    "  {}  {:>8}  {:>8}  {:>8}  {}:{}",
                    color.paint(format!("{:>8.1}x", f.amplification)),
                    f.local_cc,
                    f.reachable_cc,
                    f.reachable_count,
                    f.file,
                    f.symbol
                ));
            }
            out.push(String::new());
        }

        if !self.top_reachable.is_empty() {
            out.push(
                Color::Yellow
                    .bold()
                    .paint("## Highest Reachable CC (deepest complexity sinks)")
                    .to_string(),
            );
            out.push(String::new());
            out.push(format!(
                "  {:<8}  {:<8}  {:<8}  symbol",
                Color::White.bold().paint("reach"),
                Color::White.bold().paint("local"),
                Color::White.bold().paint("reach#")
            ));
            for f in &self.top_reachable {
                out.push(format!(
                    "  {:>8}  {:>8}  {:>8}  {}:{}",
                    Color::Red.paint(f.reachable_cc.to_string()),
                    f.local_cc,
                    f.reachable_count,
                    f.file,
                    f.symbol
                ));
            }
            out.push(String::new());
        }

        if !self.modules.is_empty() {
            out.push(Color::Yellow.bold().paint("## Modules").to_string());
            out.push(String::new());
            let w = self
                .modules
                .iter()
                .map(|m| m.module.len())
                .max()
                .unwrap_or(20);
            out.push(format!(
                "  {:<w$}  {:>5}  {:>8}  {:>10}  {:>8}",
                Color::White.bold().paint("module"),
                Color::White.bold().paint("fns"),
                Color::White.bold().paint("avg_amp"),
                Color::White.bold().paint("max_reach"),
                Color::White.bold().paint("local_cc"),
                w = w
            ));
            for m in &self.modules {
                out.push(format!(
                    "  {:<w$}  {:>5}  {:>8.1}x {:>10}  {:>8}",
                    m.module,
                    m.function_count,
                    m.avg_amplification,
                    m.max_reachable_cc,
                    m.total_local_cc,
                    w = w
                ));
            }
        }

        out.join("\n")
    }
}

/// Key to identify a function: (file, symbol_name).
type FnKey = (String, String);

/// BFS from start to compute reachable CC sum and max CC on any path.
fn reachable_complexity(
    start: &FnKey,
    cc_map: &HashMap<FnKey, usize>,
    call_edges: &HashMap<FnKey, Vec<FnKey>>,
) -> (usize, usize, usize) {
    // returns (reachable_sum, critical_path_max, reachable_count)
    let mut visited: HashSet<FnKey> = HashSet::new();
    let mut queue = VecDeque::new();
    let mut total_cc = 0usize;
    let mut max_cc = 0usize;

    queue.push_back(start.clone());
    while let Some(key) = queue.pop_front() {
        if !visited.insert(key.clone()) {
            continue;
        }
        let local_cc = cc_map.get(&key).copied().unwrap_or(1);
        total_cc += local_cc;
        max_cc = max_cc.max(local_cc);
        if let Some(callees) = call_edges.get(&key) {
            for callee in callees {
                if !visited.contains(callee) {
                    queue.push_back(callee.clone());
                }
            }
        }
    }
    let reachable_count = visited.len().saturating_sub(1); // exclude start itself
    (total_cc, max_cc, reachable_count)
}

/// Parse call edges from all source files without an index.
/// Returns (cc_map, call_edges, total_call_edges, resolved_edges).
fn build_call_graph_from_files(
    root: &Path,
) -> (
    HashMap<FnKey, usize>,
    HashMap<FnKey, Vec<FnKey>>,
    usize,
    usize,
) {
    let all_files = crate::path_resolve::all_files(root);
    let analyzer = ComplexityAnalyzer::new();

    // First pass: collect per-file cc data and raw call edges (caller -> [callee_name])
    type RawEdge = (FnKey, String); // ((file, caller_sym), callee_name)
    type FileResult = (Vec<(FnKey, usize)>, Vec<RawEdge>);
    let per_file: Vec<FileResult> = all_files
        .par_iter()
        .filter(|f| f.kind == "file")
        .filter_map(|f| {
            let abs_path = root.join(&f.path);
            support_for_path(&abs_path)?;
            let content = std::fs::read_to_string(&abs_path).ok()?;
            if content.is_empty() {
                return None;
            }
            let rel_path = f.path.clone();
            // CC per function
            let report = analyzer.analyze(&abs_path, &content);
            let cc_entries: Vec<(FnKey, usize)> = report
                .functions
                .iter()
                .map(|fc| {
                    let key = (rel_path.clone(), fc.short_name());
                    (key, fc.complexity)
                })
                .collect();

            // Raw call edges using SymbolParser
            let raw_edges: Vec<RawEdge> = extract_call_edges(&abs_path, &content, &rel_path);

            Some((cc_entries, raw_edges))
        })
        .collect();

    let mut cc_map: HashMap<FnKey, usize> = HashMap::new();
    let mut raw_edges_all: Vec<RawEdge> = Vec::new();
    for (cc_entries, edges) in per_file {
        for (key, cc) in cc_entries {
            cc_map.insert(key, cc);
        }
        raw_edges_all.extend(edges);
    }

    // Build name-to-keys index for resolution
    let mut name_index: HashMap<String, Vec<FnKey>> = HashMap::new();
    for key in cc_map.keys() {
        name_index
            .entry(key.1.clone())
            .or_default()
            .push(key.clone());
    }

    // Resolve raw edges: best-effort match callee_name -> FnKey
    let total_call_edges = raw_edges_all.len();
    let mut resolved_edges = 0usize;
    let mut call_edges: HashMap<FnKey, Vec<FnKey>> = HashMap::new();

    for (caller_key, callee_name) in raw_edges_all {
        // Strip trailing ()  and self:: prefixes
        let name = callee_name
            .trim_end_matches("()")
            .trim_start_matches("self::")
            .trim_start_matches("Self::");
        if let Some(candidates) = name_index.get(name) {
            resolved_edges += 1;
            // Prefer same-file candidates; otherwise take first
            let target = candidates
                .iter()
                .find(|(f, _)| *f == caller_key.0)
                .or_else(|| candidates.first())
                .cloned();
            if let Some(t) = target {
                call_edges.entry(caller_key).or_default().push(t);
            }
        }
    }

    (cc_map, call_edges, total_call_edges, resolved_edges)
}

/// Extract call edges from a single file using SymbolParser.
fn extract_call_edges(
    abs_path: &Path,
    content: &str,
    rel_path: &str,
) -> Vec<((String, String), String)> {
    use normalize_facts::SymbolParser;
    use normalize_facts_core::SymbolKind;
    let mut parser = SymbolParser::new();
    let symbols = parser.parse_file(abs_path, content);
    let mut edges = Vec::new();
    for sym in &symbols {
        if sym.kind != SymbolKind::Function && sym.kind != SymbolKind::Method {
            continue;
        }
        let caller_key = (rel_path.to_string(), sym.name.clone());
        let callees = parser.find_callees_for_symbol(abs_path, content, sym);
        for (callee_name, _, _) in callees {
            edges.push((caller_key.clone(), callee_name));
        }
    }
    edges
}

/// Analyze call-complexity across the codebase.
pub fn analyze_call_complexity(
    root: &Path,
    limit: usize,
    module_limit: usize,
) -> CallComplexityReport {
    let (cc_map, call_edges, total_edges, resolved_edges) = build_call_graph_from_files(root);

    let total_functions = cc_map.len();
    let unresolved_pct = if total_edges > 0 {
        (total_edges - resolved_edges) as f64 / total_edges as f64 * 100.0
    } else {
        0.0
    };

    // Compute reachable complexity for each function (BFS per function).
    let entries: Vec<FunctionCallComplexity> = cc_map
        .par_iter()
        .map(|(key, &local_cc)| {
            let (reachable_cc, critical_path_cc, reachable_count) =
                reachable_complexity(key, &cc_map, &call_edges);
            let amplification = if local_cc > 0 {
                reachable_cc as f64 / local_cc as f64
            } else {
                reachable_cc as f64
            };
            FunctionCallComplexity {
                file: key.0.clone(),
                symbol: key.1.clone(),
                local_cc,
                reachable_cc,
                critical_path_cc,
                amplification,
                reachable_count,
            }
        })
        .collect();

    // Per-module aggregation over the full set.
    // (amp_sum, max_reachable, local_sum, count)
    let mut module_acc: BTreeMap<String, (f64, usize, usize, usize)> = BTreeMap::new();
    for e in &entries {
        let key = module_key(&e.file);
        let acc = module_acc.entry(key).or_default();
        acc.0 += e.amplification;
        acc.1 = acc.1.max(e.reachable_cc);
        acc.2 += e.local_cc;
        acc.3 += 1;
    }
    let mut modules: Vec<ModuleCallComplexity> = module_acc
        .into_iter()
        .filter(|(_, (_, _, _, count))| *count > 0)
        .map(
            |(module, (amp_sum, max_reach, local_sum, count))| ModuleCallComplexity {
                module,
                avg_amplification: amp_sum / count as f64,
                max_reachable_cc: max_reach,
                total_local_cc: local_sum,
                function_count: count,
            },
        )
        .collect();
    modules.sort_by(|a, b| {
        b.total_local_cc
            .cmp(&a.total_local_cc)
            .then_with(|| b.max_reachable_cc.cmp(&a.max_reachable_cc))
    });
    if module_limit > 0 {
        modules.truncate(module_limit);
    }

    // Top amplified: highest amplification ratio.
    let mut by_amplification = entries.clone();
    by_amplification.sort_by(|a, b| {
        b.amplification
            .partial_cmp(&a.amplification)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.reachable_cc.cmp(&a.reachable_cc))
    });
    by_amplification.truncate(limit);

    // Top reachable: highest absolute reachable CC.
    let mut by_reachable = entries;
    by_reachable.sort_by(|a, b| {
        b.reachable_cc.cmp(&a.reachable_cc).then_with(|| {
            b.amplification
                .partial_cmp(&a.amplification)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    by_reachable.truncate(limit);

    CallComplexityReport {
        root: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        index_available: false,
        total_functions,
        unresolved_callees_pct: unresolved_pct,
        top_amplified: by_amplification,
        top_reachable: by_reachable,
        modules,
    }
}
