//! Architecture analysis: coupling and structural insights
//!
//! Provides insights by default - no configuration needed.
//! Cycle detection lives in `graph.rs` (Tarjan SCC) — use `analyze graph`.

use crate::index::FileIndex;
use crate::output::OutputFormatter;
use normalize_analyze::truncate_path;
pub use normalize_architecture::{
    CrossImport, Cycle, HubModule, ImportChain, ImportGraph, LayerFlow, ModuleCoupling,
    OrphanModule, SymbolMetrics, build_import_graph,
};
use normalize_architecture::{
    compute_coupling_and_hubs, compute_layer_flows, detect_cross_imports, find_orphan_modules,
    find_symbol_hotspots,
};
use normalize_languages::is_programming_language;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

/// Full architecture analysis report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ArchitectureReport {
    pub cross_imports: Vec<CrossImport>,
    pub hub_modules: Vec<HubModule>,
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

    let coupling_and_hubs =
        compute_coupling_and_hubs(&graph.imports_by_file, &graph.importers_by_file, &all_files);
    let cross_imports = detect_cross_imports(&graph.imports_by_file);
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
        cross_imports,
        hub_modules: coupling_and_hubs.hubs,
        layer_flows,
        coupling_hotspots: coupling_and_hubs.coupling,
        orphan_modules: orphans,
        symbol_hotspots,
        total_modules,
        total_symbols: total_symbols as usize,
        total_imports,
        resolved_imports,
    })
}
