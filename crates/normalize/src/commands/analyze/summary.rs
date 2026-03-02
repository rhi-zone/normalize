//! Codebase summary: single-page overview aggregating health, budget, module health, and architecture.

use crate::commands::analyze::architecture::{ArchitectureReport, analyze_architecture};
use crate::commands::analyze::budget::{BudgetReport, analyze_budget};
use crate::commands::analyze::module_health::{ModuleHealthReport, analyze_module_health};
use crate::health::{HealthReport, analyze_health};
use crate::output::OutputFormatter;
use serde::Serialize;
use std::path::Path;

/// A top concern surfaced by the summary.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Concern {
    pub description: String,
    pub detail: String,
}

/// Architecture stats extracted from `ArchitectureReport` (or defaults if index unavailable).
#[derive(Debug, Default, Serialize, schemars::JsonSchema)]
pub struct ArchStats {
    pub hubs: usize,
}

/// Single-page codebase overview.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SummaryReport {
    pub project: String,
    pub health: HealthReport,
    pub budget: BudgetReport,
    pub module_health: ModuleHealthReport,
    pub arch: ArchStats,
    pub concerns: Vec<Concern>,
}

fn extract_arch_stats(report: &ArchitectureReport) -> ArchStats {
    ArchStats {
        hubs: report.hub_modules.len(),
    }
}

fn build_concerns(health: &HealthReport, module_health: &ModuleHealthReport) -> Vec<Concern> {
    let mut concerns = Vec::new();

    // Worst module
    if let Some(worst) = module_health.modules.first() {
        concerns.push(Concern {
            description: format!("{} — {:.0}% health", worst.module, worst.score * 100.0),
            detail: format!(
                "{:.0}% test, {:.3} density",
                worst.test_ratio * 100.0,
                worst.density_score,
            ),
        });
    }

    // Massive files
    for lf in health.large_files.iter().take(2) {
        if lf.lines >= 2000 {
            concerns.push(Concern {
                description: format!("{} — {} lines", lf.path, lf.lines),
                detail: "massive file".to_string(),
            });
        }
    }

    // Highest complexity functions
    if let Some(top) = health.top_offenders.first() {
        let loc = match (&top.file_path, top.start_line) {
            (Some(p), l) => format!("{}:{}", p, l),
            (None, l) => format!(":{}", l),
        };
        concerns.push(Concern {
            description: format!("{} — complexity {}", top.name, top.complexity),
            detail: loc,
        });
    }

    concerns.truncate(5);
    concerns
}

/// Run the full summary analysis.
pub fn analyze_summary(root: &Path, module_limit: usize) -> SummaryReport {
    let ((health, budget), module_health) = rayon::join(
        || rayon::join(|| analyze_health(root), || analyze_budget(root, 0)),
        || analyze_module_health(root, module_limit, 100),
    );

    // Architecture requires the facts index — best-effort.
    let arch = try_architecture(root).unwrap_or_default();

    let concerns = build_concerns(&health, &module_health);

    let project = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());

    SummaryReport {
        project,
        health,
        budget,
        module_health,
        arch,
        concerns,
    }
}

fn try_architecture(root: &Path) -> Option<ArchStats> {
    let rt = tokio::runtime::Runtime::new().ok()?;
    let idx = rt.block_on(crate::index::ensure_ready(root)).ok()?;
    let report = rt.block_on(analyze_architecture(&idx)).ok()?;
    Some(extract_arch_stats(&report))
}

fn human_lines(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

impl OutputFormatter for SummaryReport {
    fn format_text(&self) -> String {
        let mut out = Vec::new();

        let h = &self.health;
        let breakdown = h.score_breakdown();
        let grade = h.grade();
        let lang_count = h.files_by_language.len();

        out.push(format!("# Summary: {}", self.project));
        out.push(String::new());
        out.push(format!(
            "{} ({:.0}%) · {} files · {} lines · {} languages · {} functions",
            grade,
            breakdown.total * 100.0,
            h.total_files,
            human_lines(h.total_lines),
            lang_count,
            h.total_functions,
        ));

        // Composition from budget
        if self.budget.total_lines > 0 {
            out.push(String::new());
            out.push("## Composition".to_string());
            let parts: Vec<String> = self
                .budget
                .categories
                .iter()
                .filter(|c| c.pct >= 1.0)
                .map(|c| format!("{:.0}% {}", c.pct, c.category))
                .collect();
            out.push(format!("  {}", parts.join(" · ")));
        }

        // Health breakdown
        out.push(String::new());
        out.push("## Health Breakdown".to_string());
        let rows: &[(&str, f64, &str)] = &[
            (
                "complexity",
                breakdown.complexity,
                &breakdown.complexity_reason,
            ),
            ("risk", breakdown.risk, &breakdown.risk_reason),
            (
                "file sizes",
                breakdown.file_size,
                &breakdown.file_size_reason,
            ),
            (
                "test ratio",
                breakdown.test_coverage,
                &breakdown.test_coverage_reason,
            ),
            ("ceremony", breakdown.ceremony, &breakdown.ceremony_reason),
            (
                "duplicates",
                breakdown.duplicates,
                &breakdown.duplicates_reason,
            ),
            (
                "uniqueness",
                breakdown.uniqueness,
                &breakdown.uniqueness_reason,
            ),
        ];
        for (label, score, reason) in rows {
            out.push(format!("  {:<12} {:.0}%  {}", label, score * 100.0, reason));
        }

        // Top concerns
        if !self.concerns.is_empty() {
            out.push(String::new());
            out.push("## Top Concerns".to_string());
            for (i, c) in self.concerns.iter().enumerate() {
                out.push(format!("  {}. {} ({})", i + 1, c.description, c.detail));
            }
        }

        // Architecture
        if self.arch.hubs > 0 {
            out.push(String::new());
            out.push("## Architecture".to_string());
            out.push(format!("  {} hub modules", self.arch.hubs));
        }

        out.join("\n")
    }

    fn format_pretty(&self) -> String {
        use normalize_output::progress_bar;
        use nu_ansi_term::{Color, Style};

        let mut out = Vec::new();

        let h = &self.health;
        let breakdown = h.score_breakdown();
        let grade = h.grade();
        let lang_count = h.files_by_language.len();

        let grade_color = match grade {
            "A" => Color::Purple,
            "B" => Color::Blue,
            "C" => Color::Green,
            "D" => Color::Yellow,
            _ => Color::Red,
        };

        out.push(
            Style::new()
                .bold()
                .paint(format!("Summary: {}", self.project))
                .to_string(),
        );
        out.push(String::new());
        out.push(format!(
            "{} ({:.0}%) · {} files · {} lines · {} languages · {} functions",
            grade_color.bold().paint(grade),
            breakdown.total * 100.0,
            h.total_files,
            human_lines(h.total_lines),
            lang_count,
            h.total_functions,
        ));

        // Composition
        if self.budget.total_lines > 0 {
            out.push(String::new());
            out.push(Style::new().bold().paint("Composition").to_string());
            let parts: Vec<String> = self
                .budget
                .categories
                .iter()
                .filter(|c| c.pct >= 1.0)
                .map(|c| format!("{:.0}% {}", c.pct, c.category))
                .collect();
            out.push(format!("  {}", parts.join(" · ")));
        }

        // Health breakdown
        out.push(String::new());
        out.push(Style::new().bold().paint("Health Breakdown").to_string());
        let score_color = |s: f64| -> Color {
            if s >= 0.75 {
                Color::Green
            } else if s >= 0.50 {
                Color::Yellow
            } else {
                Color::Red
            }
        };
        const LW: usize = 12;
        let rows: &[(&str, f64, &str)] = &[
            (
                "complexity",
                breakdown.complexity,
                &breakdown.complexity_reason,
            ),
            ("risk", breakdown.risk, &breakdown.risk_reason),
            (
                "file sizes",
                breakdown.file_size,
                &breakdown.file_size_reason,
            ),
            (
                "test ratio",
                breakdown.test_coverage,
                &breakdown.test_coverage_reason,
            ),
            ("ceremony", breakdown.ceremony, &breakdown.ceremony_reason),
            (
                "duplicates",
                breakdown.duplicates,
                &breakdown.duplicates_reason,
            ),
            (
                "uniqueness",
                breakdown.uniqueness,
                &breakdown.uniqueness_reason,
            ),
        ];
        for (label, score, reason) in rows {
            let c = score_color(*score);
            out.push(format!(
                "  {}  {}  {}  {}",
                Style::new().dimmed().paint(format!("{:<LW$}", label)),
                c.paint(progress_bar(*score, 12)),
                c.paint(format!("{:.0}%", score * 100.0)),
                Style::new().dimmed().paint(*reason),
            ));
        }

        // Top concerns
        if !self.concerns.is_empty() {
            out.push(String::new());
            out.push(Style::new().bold().paint("Top Concerns").to_string());
            for (i, c) in self.concerns.iter().enumerate() {
                out.push(format!(
                    "  {}. {} {}",
                    Color::Yellow.paint(format!("{}", i + 1)),
                    c.description,
                    Style::new().dimmed().paint(format!("({})", c.detail)),
                ));
            }
        }

        // Architecture
        if self.arch.hubs > 0 {
            out.push(String::new());
            out.push(Style::new().bold().paint("Architecture").to_string());
            out.push(format!("  {} hub modules", self.arch.hubs));
        }

        out.join("\n")
    }
}
