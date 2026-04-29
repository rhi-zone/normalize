//! Codebase health metrics.
//!
//! Quick overview of codebase health including file counts,
//! complexity summary, and structural metrics.

use glob::Pattern;
use normalize_output::OutputFormatter;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use crate::commands::analyze::ceremony::analyze_ceremony;
use crate::commands::analyze::complexity::analyze_codebase_complexity;
use crate::commands::analyze::density::analyze_density;
use crate::commands::analyze::duplicates::{
    DuplicateFunctionsConfig, build_duplicate_functions_report,
};
use crate::commands::analyze::test_ratio::analyze_test_ratio;
use crate::commands::analyze::uniqueness::analyze_uniqueness;
use crate::index::FileIndex;

/// Large file info for reporting
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct LargeFile {
    pub path: String,
    pub lines: usize,
}

/// Thresholds for file size severity
const LARGE_THRESHOLD: usize = 500;
const VERY_LARGE_THRESHOLD: usize = 1000;
const MASSIVE_THRESHOLD: usize = 2000;

/// Health metrics for a codebase
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct HealthReport {
    pub total_files: usize,
    pub files_by_language: HashMap<String, usize>,
    pub total_lines: usize,
    pub total_complexity: usize,
    pub avg_complexity: f64,
    pub max_complexity: usize,
    pub high_risk_functions: usize,
    pub total_functions: usize,
    pub large_files: Vec<LargeFile>,
    /// Languages present in the codebase whose tree-sitter grammar is not installed,
    /// causing complexity analysis to be skipped for those files.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_grammars: Vec<String>,
    /// Top complexity offenders (up to 5), sorted by complexity descending.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub top_offenders: Vec<crate::analyze::complexity::FunctionComplexity>,
    /// Test LOC / total LOC ratio (0.0–1.0), if computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_ratio: Option<f64>,
    /// Interface-impl / total callables ratio (0.0–1.0), if computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ceremony_ratio: Option<f64>,
    /// Number of exact-duplicate function groups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_groups: Option<usize>,
    /// Average density score (0.0–1.0): (compression_ratio + token_uniqueness) / 2.
    /// Lower = more repetitive. Informational only — not scored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density_score: Option<f64>,
    /// Fraction of functions with no structural near-twin (0.0–1.0).
    /// Higher = more unique. Contributes to health score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uniqueness_ratio: Option<f64>,
    /// Lines of code involved in duplicates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicated_lines: Option<usize>,
}

impl OutputFormatter for HealthReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();

        let breakdown = self.score_breakdown();
        let grade = self.grade();

        lines.push(format!(
            "# Codebase Health  {} ({:.0}%)",
            grade,
            breakdown.total * 100.0
        ));
        lines.push(format!(
            "  complexity  {:.0}%  {}",
            breakdown.complexity * 100.0,
            breakdown.complexity_reason
        ));
        lines.push(format!(
            "  risk        {:.0}%  {}",
            breakdown.risk * 100.0,
            breakdown.risk_reason
        ));
        lines.push(format!(
            "  file sizes  {:.0}%  {}",
            breakdown.file_size * 100.0,
            breakdown.file_size_reason
        ));
        lines.push(format!(
            "  test ratio  {:.0}%  {}",
            breakdown.test_coverage * 100.0,
            breakdown.test_coverage_reason
        ));
        lines.push(format!(
            "  ceremony    {:.0}%  {}",
            breakdown.ceremony * 100.0,
            breakdown.ceremony_reason
        ));
        lines.push(format!(
            "  duplicates  {:.0}%  {}",
            breakdown.duplicates * 100.0,
            breakdown.duplicates_reason
        ));
        lines.push(format!(
            "  uniqueness  {:.0}%  {}",
            breakdown.uniqueness * 100.0,
            breakdown.uniqueness_reason
        ));
        if let Some(d) = self.density_score {
            lines.push(format!(
                "  density     {:.3}  (compression+token avg, lower = more repetitive)",
                d
            ));
        }
        lines.push(String::new());

        lines.push("## Files".to_string());
        let human_lines = if self.total_lines >= 1_000_000 {
            format!("{:.1}M lines", self.total_lines as f64 / 1_000_000.0)
        } else if self.total_lines >= 1_000 {
            format!("{:.0}K lines", self.total_lines as f64 / 1_000.0)
        } else {
            format!("{} lines", self.total_lines)
        };
        lines.push(format!("  {} files · {}", self.total_files, human_lines));
        let mut by_language: Vec<_> = self.files_by_language.iter().collect();
        by_language.sort_by(|a, b| b.1.cmp(a.1));
        for (lang, count) in &by_language {
            if **count > 0 {
                lines.push(format!("  {}: {}", lang, count));
            }
        }
        lines.push(String::new());

        lines.push("## Complexity".to_string());
        if !self.missing_grammars.is_empty() {
            lines.push(format!(
                "  Warning: grammar not installed for {} — run `normalize grammars install`",
                self.missing_grammars.join(", ")
            ));
        } else if self.total_functions > 0 {
            let high_ratio = self.high_risk_functions as f64 / self.total_functions as f64;
            lines.push(format!(
                "  {} functions · total {} · avg {:.1} · max {} · {:.0}% high-risk",
                self.total_functions,
                self.total_complexity,
                self.avg_complexity,
                self.max_complexity,
                high_ratio * 100.0,
            ));
            for f in &self.top_offenders {
                let loc = match (&f.file_path, f.start_line) {
                    (Some(p), l) => format!("{}:{}", p, l),
                    (None, l) => format!(":{}", l),
                };
                lines.push(format!("  {}  {} ({})", f.complexity, f.name, loc));
            }
        } else {
            lines.push("  No functions found".to_string());
        }

        // Large files — show only the worst tier present
        let massive: Vec<_> = self
            .large_files
            .iter()
            .filter(|f| f.lines >= MASSIVE_THRESHOLD)
            .collect();
        let very_large: Vec<_> = self
            .large_files
            .iter()
            .filter(|f| f.lines >= VERY_LARGE_THRESHOLD && f.lines < MASSIVE_THRESHOLD)
            .collect();
        let large: Vec<_> = self
            .large_files
            .iter()
            .filter(|f| f.lines >= LARGE_THRESHOLD && f.lines < VERY_LARGE_THRESHOLD)
            .collect();

        if !massive.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "## CRITICAL: Massive Files (>{} lines) — {}",
                MASSIVE_THRESHOLD,
                massive.len()
            ));
            for lf in massive.iter().take(10) {
                lines.push(format!("  {} ({} lines)", lf.path, lf.lines));
            }
            if massive.len() > 10 {
                lines.push(format!("  … and {} more", massive.len() - 10));
            }
        } else if !very_large.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "## WARNING: Very Large Files (>{} lines) — {}",
                VERY_LARGE_THRESHOLD,
                very_large.len()
            ));
            for lf in very_large.iter().take(5) {
                lines.push(format!("  {} ({} lines)", lf.path, lf.lines));
            }
            if very_large.len() > 5 {
                lines.push(format!("  … and {} more", very_large.len() - 5));
            }
        } else if !large.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "## Large Files (>{} lines) — {}",
                LARGE_THRESHOLD,
                large.len()
            ));
            for lf in large.iter().take(5) {
                lines.push(format!("  {} ({} lines)", lf.path, lf.lines));
            }
            if large.len() > 5 {
                lines.push(format!("  … and {} more", large.len() - 5));
            }
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        use normalize_output::progress_bar;
        use nu_ansi_term::{Color, Style};

        let mut lines = Vec::new();
        let breakdown = self.score_breakdown();
        let health_score = breakdown.total;
        let grade = self.grade();

        lines.push(Style::new().bold().paint("Codebase Health").to_string());
        lines.push(String::new());

        // Health score: show grades A–F as a ladder.
        // Unreached grades above: colorized + dimmed, empty bar, 0%.
        // Current grade: bold color, bar filled to show progress within the tier.
        // Grades below: hidden.
        lines.push(format!(
            "{}  {:.0}%",
            Style::new().bold().paint("Health Score"),
            health_score * 100.0
        ));
        // (label, tier_lower, tier_upper, color)
        let grade_thresholds: &[(&str, f64, f64, Color)] = &[
            ("A", 0.9, 1.0, Color::Purple),
            ("B", 0.8, 0.9, Color::Blue),
            ("C", 0.7, 0.8, Color::Green),
            ("D", 0.6, 0.7, Color::Yellow),
            ("F", 0.0, 0.6, Color::Red),
        ];
        const LW: usize = 12; // label column width (matches breakdown labels below)
        for &(g, lower, upper, color) in grade_thresholds {
            if g == grade {
                let within = (health_score - lower) / (upper - lower);
                lines.push(format!(
                    "  {}  {}  {:.0}%",
                    color.bold().paint(format!("{:<LW$}", g)),
                    color.paint(progress_bar(within, 12)),
                    within * 100.0
                ));
                break; // hide grades below
            } else {
                // Unreached: dimmed color, empty bar, 0%
                let dimmed = color.dimmed();
                lines.push(format!(
                    "  {}  {}  0%",
                    dimmed.paint(format!("{:<LW$}", g)),
                    dimmed.paint(progress_bar(0.0, 12)),
                ));
            }
        }
        lines.push(String::new());
        // Score breakdown — pad label in plain text before styling so ANSI codes
        // don't confuse format!'s width measurement.
        // Bar color reflects the score for that dimension.
        let score_color = |score: f64| -> Color {
            if score >= 0.75 {
                Color::Green
            } else if score >= 0.50 {
                Color::Yellow
            } else {
                Color::Red
            }
        };
        let score_row = |label: &str, score: f64, reason: &str| {
            let c = score_color(score);
            format!(
                "  {}  {}  {}  {}",
                Style::new().dimmed().paint(format!("{:<LW$}", label)),
                c.paint(progress_bar(score, 12)),
                c.paint(format!("{:.0}%", score * 100.0)),
                Style::new().dimmed().paint(reason),
            )
        };
        lines.push(score_row(
            "complexity",
            breakdown.complexity,
            &breakdown.complexity_reason,
        ));
        lines.push(score_row("risk", breakdown.risk, &breakdown.risk_reason));
        lines.push(score_row(
            "file sizes",
            breakdown.file_size,
            &breakdown.file_size_reason,
        ));
        lines.push(score_row(
            "test ratio",
            breakdown.test_coverage,
            &breakdown.test_coverage_reason,
        ));
        lines.push(score_row(
            "ceremony",
            breakdown.ceremony,
            &breakdown.ceremony_reason,
        ));
        lines.push(score_row(
            "duplicates",
            breakdown.duplicates,
            &breakdown.duplicates_reason,
        ));
        lines.push(score_row(
            "uniqueness",
            breakdown.uniqueness,
            &breakdown.uniqueness_reason,
        ));
        if let Some(d) = self.density_score {
            let d_color = if d >= 0.45 {
                Color::Green
            } else if d >= 0.35 {
                Color::Yellow
            } else {
                Color::Red
            };
            lines.push(format!(
                "  {}  {}  (compression+token avg, lower = more repetitive)",
                Style::new().dimmed().paint(format!("{:<LW$}", "density")),
                d_color.paint(format!("{:.3}", d)),
            ));
        }
        lines.push(String::new());

        // Files section
        lines.push(Style::new().bold().paint("Files").to_string());
        let human_lines = if self.total_lines >= 1_000_000 {
            format!("{:.1}M lines", self.total_lines as f64 / 1_000_000.0)
        } else if self.total_lines >= 1_000 {
            format!("{:.0}K lines", self.total_lines as f64 / 1_000.0)
        } else {
            format!("{} lines", self.total_lines)
        };
        lines.push(format!("  {} files · {}", self.total_files, human_lines));

        // Language bars (normalized to the largest language)
        let max_count = self.files_by_language.values().max().copied().unwrap_or(1);
        let mut by_language: Vec<_> = self.files_by_language.iter().collect();
        by_language.sort_by(|a, b| b.1.cmp(a.1));
        for (lang, count) in by_language.iter().filter(|(_, c)| **c > 0) {
            let ratio = **count as f64 / max_count as f64;
            lines.push(format!(
                "  {:<16} {}  {}",
                lang,
                progress_bar(ratio, 16),
                count
            ));
        }
        lines.push(String::new());

        // Complexity section
        lines.push(Style::new().bold().paint("Complexity").to_string());
        if !self.missing_grammars.is_empty() {
            lines.push(format!(
                "  {}",
                Color::Yellow.paint(format!(
                    "grammar not installed for {} — run `normalize grammars install`",
                    self.missing_grammars.join(", ")
                ))
            ));
        } else if self.total_functions > 0 {
            lines.push(format!(
                "  {} functions · total {} · avg {:.1} · max {}",
                self.total_functions,
                self.total_complexity,
                self.avg_complexity,
                self.max_complexity
            ));
            let low_risk = self
                .total_functions
                .saturating_sub(self.high_risk_functions);
            let low_ratio = low_risk as f64 / self.total_functions as f64;
            let high_ratio = self.high_risk_functions as f64 / self.total_functions as f64;
            lines.push(format!(
                "  {}  {}  {} ({:.0}%)",
                Color::Green.paint(format!("{:<9}", "Low")),
                Color::Green.paint(progress_bar(low_ratio, 16)),
                low_risk,
                low_ratio * 100.0
            ));
            if self.high_risk_functions > 0 {
                lines.push(format!(
                    "  {}  {}  {} ({:.0}%)",
                    Color::Yellow.bold().paint(format!("{:<9}", "High risk")),
                    Color::Yellow.paint(progress_bar(high_ratio, 16)),
                    self.high_risk_functions,
                    high_ratio * 100.0
                ));
                // Top offenders
                let max_c = self
                    .top_offenders
                    .first()
                    .map(|f| f.complexity)
                    .unwrap_or(1);
                for f in &self.top_offenders {
                    let ratio = f.complexity as f64 / max_c as f64;
                    let loc = match (&f.file_path, f.start_line) {
                        (Some(p), l) => format!("{}:{}", p, l),
                        (None, l) => format!(":{}", l),
                    };
                    lines.push(format!(
                        "    {}  {}  {}",
                        Color::Yellow.paint(progress_bar(ratio, 12)),
                        Color::Yellow.paint(format!("{:>5}", f.complexity)),
                        Style::new().dimmed().paint(format!("{} ({})", f.name, loc)),
                    ));
                }
            }
        } else {
            lines.push("  No functions found".to_string());
        }

        // Large files — show only the worst tier present ("hiding ranks below")
        let massive: Vec<_> = self
            .large_files
            .iter()
            .filter(|f| f.lines >= MASSIVE_THRESHOLD)
            .collect();
        let very_large: Vec<_> = self
            .large_files
            .iter()
            .filter(|f| f.lines >= VERY_LARGE_THRESHOLD && f.lines < MASSIVE_THRESHOLD)
            .collect();
        let large: Vec<_> = self
            .large_files
            .iter()
            .filter(|f| f.lines >= LARGE_THRESHOLD && f.lines < VERY_LARGE_THRESHOLD)
            .collect();

        let fmt_file_rows =
            |out: &mut Vec<String>, files: &[&LargeFile], color: Color, limit: usize| {
                let max_lines = files.iter().map(|f| f.lines).max().unwrap_or(1);
                for lf in files.iter().take(limit) {
                    let ratio = lf.lines as f64 / max_lines as f64;
                    out.push(format!(
                        "  {:<36} {}  {}",
                        lf.path,
                        color.paint(progress_bar(ratio, 12)),
                        lf.lines
                    ));
                }
                if files.len() > limit {
                    out.push(format!("  … and {} more", files.len() - limit));
                }
            };

        if !massive.is_empty() {
            lines.push(String::new());
            lines.push(
                Color::Red
                    .bold()
                    .paint(format!(
                        "CRITICAL: Massive Files (>{} lines) — {}",
                        MASSIVE_THRESHOLD,
                        massive.len()
                    ))
                    .to_string(),
            );
            fmt_file_rows(&mut lines, &massive, Color::Red, 10);
        } else if !very_large.is_empty() {
            lines.push(String::new());
            lines.push(
                Color::Yellow
                    .bold()
                    .paint(format!(
                        "WARNING: Very Large Files (>{} lines) — {}",
                        VERY_LARGE_THRESHOLD,
                        very_large.len()
                    ))
                    .to_string(),
            );
            fmt_file_rows(&mut lines, &very_large, Color::Yellow, 5);
        } else if !large.is_empty() {
            lines.push(String::new());
            lines.push(
                Color::Blue
                    .paint(format!(
                        "Large Files (>{} lines) — {}",
                        LARGE_THRESHOLD,
                        large.len()
                    ))
                    .to_string(),
            );
            fmt_file_rows(&mut lines, &large, Color::Blue, 5);
        }

        lines.join("\n")
    }
}

pub struct HealthScoreBreakdown {
    /// Weighted total (0–1)
    pub total: f64,
    /// Avg-complexity component (0–1), weight 15%
    pub complexity: f64,
    /// High-risk-ratio component (0–1), weight 15%
    pub risk: f64,
    /// File-size component (0–1), weight 20%
    pub file_size: f64,
    /// Test ratio component (0–1), weight 20%
    pub test_coverage: f64,
    /// Ceremony ratio component (0–1), weight 5%
    pub ceremony: f64,
    /// Duplicates component (0–1), weight 15%
    pub duplicates: f64,
    /// Structural uniqueness component (0–1), weight 10%
    pub uniqueness: f64,
    /// Human-readable reason for the complexity score
    pub complexity_reason: String,
    /// Human-readable reason for the risk score
    pub risk_reason: String,
    /// Human-readable reason for the file-size score
    pub file_size_reason: String,
    /// Human-readable reason for the test coverage score
    pub test_coverage_reason: String,
    /// Human-readable reason for the ceremony score
    pub ceremony_reason: String,
    /// Human-readable reason for the duplicates score
    pub duplicates_reason: String,
    /// Human-readable reason for the uniqueness score
    pub uniqueness_reason: String,
}

impl HealthReport {
    pub fn score_breakdown(&self) -> HealthScoreBreakdown {
        // Empty codebase: no data to score — return zero rather than perfect defaults
        if self.total_files == 0 {
            return HealthScoreBreakdown {
                total: 0.0,
                complexity: 0.0,
                risk: 0.0,
                file_size: 0.0,
                test_coverage: 0.0,
                ceremony: 0.0,
                duplicates: 0.0,
                uniqueness: 0.0,
                complexity_reason: "no files".to_string(),
                risk_reason: "no files".to_string(),
                file_size_reason: "no files".to_string(),
                test_coverage_reason: "no files".to_string(),
                ceremony_reason: "no files".to_string(),
                duplicates_reason: "no files".to_string(),
                uniqueness_reason: "no files".to_string(),
            };
        }

        let complexity_score = if self.avg_complexity <= 3.0 {
            1.0
        } else if self.avg_complexity <= 5.0 {
            0.9
        } else if self.avg_complexity <= 7.0 {
            0.8
        } else if self.avg_complexity <= 10.0 {
            0.7
        } else if self.avg_complexity <= 15.0 {
            0.5
        } else {
            0.3
        };
        let complexity_reason = format!("avg complexity {:.1}", self.avg_complexity);

        let high_risk_ratio = if self.total_functions > 0 {
            self.high_risk_functions as f64 / self.total_functions as f64
        } else {
            0.0
        };
        let risk_score = if high_risk_ratio <= 0.01 {
            1.0
        } else if high_risk_ratio <= 0.02 {
            0.9
        } else if high_risk_ratio <= 0.03 {
            0.8
        } else if high_risk_ratio <= 0.05 {
            0.7
        } else if high_risk_ratio <= 0.1 {
            0.5
        } else {
            0.3
        };
        let risk_reason = format!("{:.0}% high-risk functions", high_risk_ratio * 100.0);

        let massive_count = self
            .large_files
            .iter()
            .filter(|f| f.lines >= MASSIVE_THRESHOLD)
            .count();
        let very_large_count = self
            .large_files
            .iter()
            .filter(|f| f.lines >= VERY_LARGE_THRESHOLD && f.lines < MASSIVE_THRESHOLD)
            .count();
        let file_size_score = if massive_count > 0 {
            0.3_f64.max(0.5 - (massive_count as f64 * 0.1))
        } else if very_large_count > 5 {
            0.5
        } else if very_large_count > 0 {
            0.7
        } else {
            1.0
        };
        let file_size_reason = if massive_count > 0 {
            format!(
                "{} massive file{}",
                massive_count,
                if massive_count == 1 { "" } else { "s" }
            )
        } else if very_large_count > 0 {
            format!(
                "{} very large file{}",
                very_large_count,
                if very_large_count == 1 { "" } else { "s" }
            )
        } else {
            "no oversized files".to_string()
        };

        // Test ratio scoring (higher = better)
        let test_ratio_val = self.test_ratio.unwrap_or(0.0);
        let test_coverage_score = if test_ratio_val >= 0.30 {
            1.0
        } else if test_ratio_val >= 0.20 {
            0.9
        } else if test_ratio_val >= 0.10 {
            0.7
        } else if test_ratio_val >= 0.05 {
            0.5
        } else {
            0.3
        };
        let test_coverage_reason = if self.test_ratio.is_some() {
            format!("{:.0}% test LOC", test_ratio_val * 100.0)
        } else {
            "not computed".to_string()
        };

        // Ceremony scoring (lower = better)
        let ceremony_val = self.ceremony_ratio.unwrap_or(0.0);
        let ceremony_score = if ceremony_val <= 0.20 {
            1.0
        } else if ceremony_val <= 0.30 {
            0.9
        } else if ceremony_val <= 0.40 {
            0.8
        } else if ceremony_val <= 0.50 {
            0.7
        } else if ceremony_val <= 0.60 {
            0.5
        } else {
            0.3
        };
        let ceremony_reason = if self.ceremony_ratio.is_some() {
            format!("{:.0}% boilerplate", ceremony_val * 100.0)
        } else {
            "not computed".to_string()
        };

        // Duplicates scoring: 1/(1 + groups/10) — smooth decay, 0 groups → 1.0,
        // 10 groups → 0.5, 50 groups → 0.17
        let dup_groups = self.duplicate_groups.unwrap_or(0);
        let duplicates_score = 1.0 / (1.0 + dup_groups as f64 / 10.0);
        let duplicates_reason = if self.duplicate_groups.is_some() {
            if dup_groups == 0 {
                "no duplicates".to_string()
            } else {
                format!("{} duplicate groups", dup_groups)
            }
        } else {
            "not computed".to_string()
        };

        // Uniqueness scoring (higher = more unique functions)
        let uniqueness_val = self.uniqueness_ratio.unwrap_or(1.0);
        let uniqueness_score = if uniqueness_val >= 0.95 {
            1.0
        } else if uniqueness_val >= 0.90 {
            0.9
        } else if uniqueness_val >= 0.80 {
            0.7
        } else if uniqueness_val >= 0.70 {
            0.5
        } else {
            0.3
        };
        let uniqueness_reason = if self.uniqueness_ratio.is_some() {
            format!("{:.0}% structurally unique", uniqueness_val * 100.0)
        } else {
            "not computed".to_string()
        };

        // Weights: complexity 15%, risk 15%, file_size 20%, test 20%,
        //          ceremony 5%, duplicates 15%, uniqueness 10%  → total 100%
        let total = (complexity_score * 0.15)
            + (risk_score * 0.15)
            + (file_size_score * 0.20)
            + (test_coverage_score * 0.20)
            + (ceremony_score * 0.05)
            + (duplicates_score * 0.15)
            + (uniqueness_score * 0.10);
        HealthScoreBreakdown {
            total,
            complexity: complexity_score,
            risk: risk_score,
            file_size: file_size_score,
            test_coverage: test_coverage_score,
            ceremony: ceremony_score,
            duplicates: duplicates_score,
            uniqueness: uniqueness_score,
            complexity_reason,
            risk_reason,
            file_size_reason,
            test_coverage_reason,
            ceremony_reason,
            duplicates_reason,
            uniqueness_reason,
        }
    }

    pub fn calculate_health_score(&self) -> f64 {
        self.score_breakdown().total
    }

    pub fn grade(&self) -> &'static str {
        let score = self.calculate_health_score();
        if score >= 0.9 {
            "A"
        } else if score >= 0.8 {
            "B"
        } else if score >= 0.7 {
            "C"
        } else if score >= 0.6 {
            "D"
        } else {
            "F"
        }
    }
}

/// Check if a path is a lockfile (generated, not a code smell)
fn is_lockfile(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(
        name,
        "uv.lock"
            | "Cargo.lock"
            | "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lockb"
            | "bun.lock"
            | "poetry.lock"
            | "Pipfile.lock"
            | "Gemfile.lock"
            | "composer.lock"
            | "go.sum"
            | "flake.lock"
            | "packages.lock.json" // NuGet
            | "paket.lock"
            | "pubspec.lock" // Dart/Flutter
            | "mix.lock" // Elixir
            | "rebar.lock" // Erlang
            | "Podfile.lock" // CocoaPods
            | "shrinkwrap.yaml" // pnpm
            | "deno.lock" // Deno
            | "gradle.lockfile" // Gradle
    )
}

/// Load patterns from an allow file (.normalize/large-files-allow or similar)
fn load_allow_patterns(root: &Path, filename: &str) -> Vec<Pattern> {
    let path = root.join(".normalize").join(filename);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .filter_map(|line| Pattern::new(line.trim()).ok())
        .collect()
}

/// Check if path matches any allow pattern
fn is_allowed(path: &str, patterns: &[Pattern]) -> bool {
    patterns.iter().any(|p| p.matches(path))
}

struct ComplexityStats {
    total_functions: usize,
    total_complexity: usize,
    avg_complexity: f64,
    max_complexity: usize,
    high_risk_functions: usize,
    missing_grammars: Vec<String>,
    top_offenders: Vec<crate::analyze::complexity::FunctionComplexity>,
}

fn compute_complexity_stats(root: &Path, allowlist: &[String]) -> ComplexityStats {
    use normalize_languages::parsers::parser_for;
    use normalize_path_resolve::all_files;
    use std::collections::HashSet;

    let report = analyze_codebase_complexity(root, usize::MAX, None, None, allowlist);

    // If complexity came back empty, check whether any present languages have missing grammars.
    let missing_grammars = if report.functions.is_empty() {
        let loader = normalize_languages::parsers::grammar_loader();
        let mut seen: HashSet<&'static str> = HashSet::new();
        all_files(root, None)
            .iter()
            .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
            .filter_map(|f| normalize_languages::support_for_path(std::path::Path::new(&f.path)))
            .filter(|lang| {
                // Language has extractable functions when a tags.scm is available.
                loader.get_tags(lang.grammar_name()).is_some()
            })
            .filter(|lang| parser_for(lang.grammar_name()).is_none())
            .filter(|lang| seen.insert(lang.grammar_name()))
            .map(|lang| lang.name().to_string())
            .collect()
    } else {
        vec![]
    };

    let mut top_offenders = report.functions.clone();
    top_offenders.sort_by(|a, b| b.complexity.cmp(&a.complexity));
    top_offenders.truncate(5);

    ComplexityStats {
        total_functions: report.functions.len(),
        total_complexity: report.total_complexity(),
        avg_complexity: report.avg_complexity(),
        max_complexity: report.max_complexity(),
        high_risk_functions: report.high_risk_count() + report.critical_risk_count(),
        missing_grammars,
        top_offenders,
    }
}

/// Summary numbers extracted from the new analyses for health scoring.
struct ExtraMetrics {
    test_ratio: Option<f64>,
    ceremony_ratio: Option<f64>,
    duplicate_groups: Option<usize>,
    duplicated_lines: Option<usize>,
    density_score: Option<f64>,
    uniqueness_ratio: Option<f64>,
}

fn compute_extra_metrics(root: &Path) -> ExtraMetrics {
    // Run all analyses in parallel — each is independent
    let root_buf = root.to_path_buf();
    let (((dup_report, density), (test, ceremony)), uniqueness) = rayon::join(
        || {
            rayon::join(
                || {
                    rayon::join(
                        || {
                            let cfg = DuplicateFunctionsConfig {
                                roots: std::slice::from_ref(&root_buf),
                                elide_identifiers: false,
                                elide_literals: false,
                                show_source: false,
                                min_lines: 4,
                                include_trait_impls: false,
                                filter: None,
                            };
                            build_duplicate_functions_report(cfg)
                        },
                        || analyze_density(root, 0, 0),
                    )
                },
                || rayon::join(|| analyze_test_ratio(root, 0), || analyze_ceremony(root, 0)),
            )
        },
        || analyze_uniqueness(root, 0.80, 10, false, false, 0, 0, None),
    );

    ExtraMetrics {
        test_ratio: Some(test.overall_ratio),
        ceremony_ratio: Some(ceremony.ceremony_ratio),
        duplicate_groups: Some(dup_report.group_count()),
        duplicated_lines: Some(dup_report.duplicated_line_count()),
        density_score: Some(
            (density.overall_compression_ratio + density.overall_token_uniqueness) / 2.0,
        ),
        uniqueness_ratio: Some(uniqueness.overall_uniqueness_ratio),
    }
}

pub fn analyze_health(root: &Path) -> HealthReport {
    let allow_patterns = load_allow_patterns(root, "large-files-allow");

    // Compute complexity and extra metrics in parallel (before entering async context)
    let (complexity, extra) = rayon::join(
        || compute_complexity_stats(root, &[]),
        || compute_extra_metrics(root),
    );

    // Try index first for file/line stats, fall back to filesystem walk
    let config = crate::config::NormalizeConfig::load(root);
    if config.index.enabled() {
        let fut = async { crate::index::open(root).await.ok() };
        let indexed = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
            Err(_) => tokio::runtime::Runtime::new()
                .expect("tokio runtime")
                .block_on(fut),
        };
        if let Some(mut index) = indexed {
            let fut2 = analyze_health_indexed(root, &mut index, &allow_patterns, complexity, extra);
            return match tokio::runtime::Handle::try_current() {
                Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut2)),
                Err(_) => tokio::runtime::Runtime::new()
                    .expect("tokio runtime")
                    .block_on(fut2),
            };
        }
    }
    analyze_health_unindexed(root, &allow_patterns, complexity, extra)
}

async fn analyze_health_indexed(
    _root: &Path,
    index: &mut FileIndex,
    allow_patterns: &[Pattern],
    complexity: ComplexityStats,
    extra: ExtraMetrics,
) -> HealthReport {
    let _ = index.incremental_refresh().await;

    let conn = index.connection();

    // Get file counts by language
    let mut files_by_language: HashMap<String, usize> = HashMap::new();
    let mut total_files = 0;

    if let Ok(mut rows) = conn
        .query("SELECT path FROM files WHERE is_dir = 0", ())
        .await
    {
        while let Ok(Some(row)) = rows.next().await {
            if let Ok(path_result) = row.get::<String>(0) {
                total_files += 1;
                let path = std::path::Path::new(&path_result);
                if let Some(lang) = normalize_languages::support_for_path(path) {
                    *files_by_language
                        .entry(lang.name().to_string())
                        .or_insert(0) += 1;
                }
            }
        }
    }

    // Get line counts from index
    let mut total_lines = 0usize;
    let mut large_files = Vec::new();

    if let Ok(files) = index.all_files().await {
        for file in files {
            if file.is_dir {
                continue;
            }
            if normalize_languages::support_for_path(std::path::Path::new(&file.path)).is_some() {
                total_lines += file.lines;
            }
            if file.lines >= LARGE_THRESHOLD
                && !is_lockfile(&file.path)
                && !is_allowed(&file.path, allow_patterns)
                && normalize_languages::support_for_path(std::path::Path::new(&file.path)).is_some()
            {
                large_files.push(LargeFile {
                    path: file.path,
                    lines: file.lines,
                });
            }
        }
    }

    large_files.sort_by(|a, b| b.lines.cmp(&a.lines));

    HealthReport {
        total_files,
        files_by_language,
        total_lines,
        total_complexity: complexity.total_complexity,
        avg_complexity: complexity.avg_complexity,
        max_complexity: complexity.max_complexity,
        high_risk_functions: complexity.high_risk_functions,
        total_functions: complexity.total_functions,
        large_files,
        missing_grammars: complexity.missing_grammars,
        top_offenders: complexity.top_offenders,
        test_ratio: extra.test_ratio,
        ceremony_ratio: extra.ceremony_ratio,
        duplicate_groups: extra.duplicate_groups,
        duplicated_lines: extra.duplicated_lines,
        density_score: extra.density_score,
        uniqueness_ratio: extra.uniqueness_ratio,
    }
}

/// Analyze health by walking the filesystem (no index available)
fn analyze_health_unindexed(
    root: &Path,
    allow_patterns: &[Pattern],
    complexity: ComplexityStats,
    extra: ExtraMetrics,
) -> HealthReport {
    use ignore::WalkBuilder;
    use rayon::prelude::*;

    let files: Vec<std::path::PathBuf> = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    let total_files = files.len();

    // Process files in parallel: read content, count lines, detect language
    struct FileInfo {
        lang: Option<String>,
        lines: usize,
        large: Option<LargeFile>,
    }

    let infos: Vec<FileInfo> = files
        .par_iter()
        .map(|path| {
            let lang = normalize_languages::support_for_path(path).map(|l| l.name().to_string());
            let (lines, large) = match std::fs::read_to_string(path) {
                Ok(content) => {
                    let lines = content.lines().count();
                    let large = if lang.is_some() && lines >= LARGE_THRESHOLD {
                        let rel_path = path.strip_prefix(root).unwrap_or(path);
                        let rel_str = rel_path.to_string_lossy();
                        if !is_lockfile(&rel_str) && !is_allowed(&rel_str, allow_patterns) {
                            Some(LargeFile {
                                path: rel_str.to_string(),
                                lines,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    (if lang.is_some() { lines } else { 0 }, large)
                }
                Err(_) => (0, None),
            };
            FileInfo { lang, lines, large }
        })
        .collect();

    let mut files_by_language: HashMap<String, usize> = HashMap::new();
    let mut total_lines = 0;
    let mut large_files = Vec::new();

    for info in infos {
        if let Some(lang) = info.lang {
            *files_by_language.entry(lang).or_insert(0) += 1;
        }
        total_lines += info.lines;
        if let Some(lf) = info.large {
            large_files.push(lf);
        }
    }

    large_files.sort_by(|a, b| b.lines.cmp(&a.lines));

    HealthReport {
        total_files,
        files_by_language,
        total_lines,
        total_complexity: complexity.total_complexity,
        avg_complexity: complexity.avg_complexity,
        max_complexity: complexity.max_complexity,
        high_risk_functions: complexity.high_risk_functions,
        total_functions: complexity.total_functions,
        large_files,
        missing_grammars: complexity.missing_grammars,
        top_offenders: complexity.top_offenders,
        test_ratio: extra.test_ratio,
        ceremony_ratio: extra.ceremony_ratio,
        duplicate_groups: extra.duplicate_groups,
        duplicated_lines: extra.duplicated_lines,
        density_score: extra.density_score,
        uniqueness_ratio: extra.uniqueness_ratio,
    }
}
