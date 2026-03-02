//! What-if impact analysis: reverse-dependency closure + blast radius

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

/// Run impact analysis CLI command.
pub fn cmd_impact(root: &Path, target: &str, json: bool) -> i32 {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let idx = match rt.block_on(crate::index::open_if_enabled(root)) {
        Some(i) => i,
        None => {
            eprintln!("Impact analysis requires the facts index. Run `normalize facts` first.");
            eprintln!("Or enable indexing: `normalize config set index.enabled true`");
            return 1;
        }
    };

    let report = match rt.block_on(analyze_impact(&idx, target)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Impact analysis failed: {}", e);
            return 1;
        }
    };

    let config = crate::config::NormalizeConfig::load(root);
    let format =
        crate::output::OutputFormat::from_cli(json, false, None, false, false, &config.pretty);
    report.print(&format);
    0
}

/// A file affected by a change to the target module.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ImpactEntry {
    pub file: String,
    pub depth: usize,
    pub has_tests: bool,
    pub fan_in: usize,
}

/// Blast radius summary statistics.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct BlastRadius {
    pub direct_count: usize,
    pub transitive_count: usize,
    pub untested_count: usize,
    pub max_depth: usize,
}

/// Full impact analysis report.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ImpactReport {
    pub target: String,
    pub direct_dependents: Vec<ImpactEntry>,
    pub transitive_dependents: Vec<ImpactEntry>,
    pub untested_paths: Vec<String>,
    pub blast_radius: BlastRadius,
}

/// Run impact analysis: BFS through reverse import graph from target.
pub async fn analyze_impact(idx: &FileIndex, target: &str) -> Result<ImpactReport, libsql::Error> {
    use super::architecture::build_import_graph;
    use super::test_ratio::is_test_file_path;

    let graph = build_import_graph(idx).await?;

    // Compute fan-in per file
    let fan_in: HashMap<&str, usize> = graph
        .importers_by_file
        .iter()
        .map(|(file, importers)| (file.as_str(), importers.len()))
        .collect();

    // BFS through reverse edges (importers_by_file)
    let mut visited: HashMap<String, usize> = HashMap::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    // Seed with direct importers of the target
    if let Some(importers) = graph.importers_by_file.get(target) {
        for importer in importers {
            if !visited.contains_key(importer) {
                visited.insert(importer.clone(), 1);
                queue.push_back((importer.clone(), 1));
            }
        }
    }

    // BFS
    while let Some((file, depth)) = queue.pop_front() {
        if let Some(importers) = graph.importers_by_file.get(&file) {
            for importer in importers {
                if !visited.contains_key(importer) && importer != target {
                    visited.insert(importer.clone(), depth + 1);
                    queue.push_back((importer.clone(), depth + 1));
                }
            }
        }
    }

    // Build entries
    let mut direct = Vec::new();
    let mut transitive = Vec::new();

    for (file, depth) in &visited {
        let entry = ImpactEntry {
            file: file.clone(),
            depth: *depth,
            has_tests: is_test_file_path(file),
            fan_in: fan_in.get(file.as_str()).copied().unwrap_or(0),
        };
        if *depth == 1 {
            direct.push(entry);
        } else {
            transitive.push(entry);
        }
    }

    // Sort: depth ascending, then fan-in descending
    direct.sort_by(|a, b| a.depth.cmp(&b.depth).then(b.fan_in.cmp(&a.fan_in)));
    transitive.sort_by(|a, b| a.depth.cmp(&b.depth).then(b.fan_in.cmp(&a.fan_in)));

    // Untested impact paths: chains from target through non-test files
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

    Ok(ImpactReport {
        target: target.to_string(),
        direct_dependents: direct,
        transitive_dependents: transitive,
        untested_paths,
        blast_radius,
    })
}

/// Build untested impact path descriptions.
/// Each path is a chain of untested files reachable from the target.
fn build_untested_paths(
    visited: &HashMap<String, usize>,
    importers_by_file: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    use super::test_ratio::is_test_file_path;

    // Collect untested files sorted by depth
    let mut untested: Vec<(&String, usize)> = visited
        .iter()
        .filter(|(f, _)| !is_test_file_path(f))
        .map(|(f, d)| (f, *d))
        .collect();
    untested.sort_by_key(|(_, d)| *d);

    // For each untested file at depth 1, trace forward through untested dependents
    let mut paths = Vec::new();
    for (file, depth) in &untested {
        if *depth != 1 {
            continue;
        }
        let mut chain = vec![format!("{} (depth 1)", file)];
        // Find untested files that import this one (depth 2+)
        if let Some(next_importers) = importers_by_file.get(*file) {
            for next in next_importers {
                if let Some(nd) = visited.get(next)
                    && !is_test_file_path(next)
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

impl OutputFormatter for ImpactReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        let total = self.blast_radius.direct_count + self.blast_radius.transitive_count;

        lines.push(format!("# Impact: {}", self.target));
        lines.push(String::new());
        lines.push(format!(
            "{} files affected · {} direct · {} transitive · {} untested · max depth {}",
            total,
            self.blast_radius.direct_count,
            self.blast_radius.transitive_count,
            self.blast_radius.untested_count,
            self.blast_radius.max_depth,
        ));

        if !self.direct_dependents.is_empty() {
            lines.push(String::new());
            lines.push(format!("## Direct ({})", self.direct_dependents.len()));
            for e in &self.direct_dependents {
                let tested = if e.has_tests { "tested" } else { "UNTESTED" };
                lines.push(format!(
                    "  {:<40} depth {}  {}  fan-in {}",
                    e.file, e.depth, tested, e.fan_in
                ));
            }
        }

        if !self.transitive_dependents.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "## Transitive ({})",
                self.transitive_dependents.len()
            ));
            for e in &self.transitive_dependents {
                let tested = if e.has_tests { "tested" } else { "UNTESTED" };
                lines.push(format!(
                    "  {:<40} depth {}  {}  fan-in {}",
                    e.file, e.depth, tested, e.fan_in
                ));
            }
        }

        if !self.untested_paths.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "## Untested Impact Paths ({})",
                self.untested_paths.len()
            ));
            for p in &self.untested_paths {
                lines.push(format!("  {}", p));
            }
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        let mut lines = Vec::new();
        let total = self.blast_radius.direct_count + self.blast_radius.transitive_count;

        lines.push(format!("\x1b[1;36m# Impact: {}\x1b[0m", self.target));
        lines.push(String::new());
        lines.push(format!(
            "\x1b[1m{}\x1b[0m files affected · \x1b[32m{}\x1b[0m direct · \x1b[33m{}\x1b[0m transitive · \x1b[31m{}\x1b[0m untested · max depth {}",
            total,
            self.blast_radius.direct_count,
            self.blast_radius.transitive_count,
            self.blast_radius.untested_count,
            self.blast_radius.max_depth,
        ));

        if !self.direct_dependents.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "\x1b[1;32m## Direct ({})\x1b[0m",
                self.direct_dependents.len()
            ));
            for e in &self.direct_dependents {
                let tested = if e.has_tests {
                    "\x1b[32mtested\x1b[0m"
                } else {
                    "\x1b[1;31mUNTESTED\x1b[0m"
                };
                lines.push(format!(
                    "  {:<40} depth {}  {}  fan-in {}",
                    e.file, e.depth, tested, e.fan_in
                ));
            }
        }

        if !self.transitive_dependents.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "\x1b[1;33m## Transitive ({})\x1b[0m",
                self.transitive_dependents.len()
            ));
            for e in &self.transitive_dependents {
                let tested = if e.has_tests {
                    "\x1b[32mtested\x1b[0m"
                } else {
                    "\x1b[1;31mUNTESTED\x1b[0m"
                };
                lines.push(format!(
                    "  {:<40} depth {}  {}  fan-in {}",
                    e.file, e.depth, tested, e.fan_in
                ));
            }
        }

        if !self.untested_paths.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "\x1b[1;31m## Untested Impact Paths ({})\x1b[0m",
                self.untested_paths.len()
            ));
            for p in &self.untested_paths {
                lines.push(format!("  {}", p));
            }
        }

        lines.join("\n")
    }
}
