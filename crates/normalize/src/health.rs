//! Codebase health metrics.
//!
//! Quick overview of codebase health including file counts,
//! complexity summary, and structural metrics.

use glob::Pattern;
use normalize_output::OutputFormatter;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

use crate::commands::analyze::complexity::analyze_codebase_complexity;
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
        for &(g, lower, upper, color) in grade_thresholds {
            if g == grade {
                let within = (health_score - lower) / (upper - lower);
                lines.push(format!(
                    "  {}  {}  {:.0}%",
                    color.bold().paint(g),
                    color.paint(progress_bar(within, 20)),
                    within * 100.0
                ));
                break; // hide grades below
            } else {
                // Unreached: dimmed color, empty bar, 0%
                let dimmed = color.dimmed();
                lines.push(format!(
                    "  {}  {}  0%",
                    dimmed.paint(g),
                    dimmed.paint(progress_bar(0.0, 20)),
                ));
            }
        }
        // Score breakdown
        lines.push(format!(
            "  {:<12}  {}  {:.0}%  {}",
            Style::new().dimmed().paint("complexity"),
            progress_bar(breakdown.complexity, 12),
            breakdown.complexity * 100.0,
            Style::new().dimmed().paint(&breakdown.complexity_reason),
        ));
        lines.push(format!(
            "  {:<12}  {}  {:.0}%  {}",
            Style::new().dimmed().paint("risk"),
            progress_bar(breakdown.risk, 12),
            breakdown.risk * 100.0,
            Style::new().dimmed().paint(&breakdown.risk_reason),
        ));
        lines.push(format!(
            "  {:<12}  {}  {:.0}%  {}",
            Style::new().dimmed().paint("file sizes"),
            progress_bar(breakdown.file_size, 12),
            breakdown.file_size * 100.0,
            Style::new().dimmed().paint(&breakdown.file_size_reason),
        ));
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

struct HealthScoreBreakdown {
    /// Weighted total (0–1)
    total: f64,
    /// Avg-complexity component (0–1), weight 30%
    complexity: f64,
    /// High-risk-ratio component (0–1), weight 30%
    risk: f64,
    /// File-size component (0–1), weight 40%
    file_size: f64,
    /// Human-readable reason for the complexity score
    complexity_reason: String,
    /// Human-readable reason for the risk score
    risk_reason: String,
    /// Human-readable reason for the file-size score
    file_size_reason: String,
}

impl HealthReport {
    fn score_breakdown(&self) -> HealthScoreBreakdown {
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

        let total = (complexity_score * 0.3) + (risk_score * 0.3) + (file_size_score * 0.4);
        HealthScoreBreakdown {
            total,
            complexity: complexity_score,
            risk: risk_score,
            file_size: file_size_score,
            complexity_reason,
            risk_reason,
            file_size_reason,
        }
    }

    fn calculate_health_score(&self) -> f64 {
        self.score_breakdown().total
    }

    fn grade(&self) -> &'static str {
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
        let mut seen: HashSet<&'static str> = HashSet::new();
        all_files(root, None)
            .iter()
            .filter(|f| f.kind == "file")
            .filter_map(|f| normalize_languages::support_for_path(std::path::Path::new(&f.path)))
            .filter(|lang| !lang.function_kinds().is_empty())
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

pub fn analyze_health(root: &Path) -> HealthReport {
    let allow_patterns = load_allow_patterns(root, "large-files-allow");

    // Compute complexity upfront (before entering async context to avoid nested runtime)
    let complexity = compute_complexity_stats(root, &[]);

    // Try index first for file/line stats, fall back to filesystem walk
    let rt = tokio::runtime::Runtime::new().unwrap();
    if let Some(mut index) = rt.block_on(crate::index::open_if_enabled(root)) {
        return rt.block_on(analyze_health_indexed(
            root,
            &mut index,
            &allow_patterns,
            complexity,
        ));
    }
    analyze_health_unindexed(root, &allow_patterns, complexity)
}

async fn analyze_health_indexed(
    _root: &Path,
    index: &mut FileIndex,
    allow_patterns: &[Pattern],
    complexity: ComplexityStats,
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
    }
}

/// Analyze health by walking the filesystem (no index available)
fn analyze_health_unindexed(
    root: &Path,
    allow_patterns: &[Pattern],
    complexity: ComplexityStats,
) -> HealthReport {
    use ignore::WalkBuilder;

    let mut files_by_language: HashMap<String, usize> = HashMap::new();
    let mut total_files = 0;
    let mut total_lines = 0;
    let mut large_files = Vec::new();

    let walker = WalkBuilder::new(root).hidden(true).git_ignore(true).build();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        total_files += 1;

        if let Some(lang) = normalize_languages::support_for_path(path) {
            *files_by_language
                .entry(lang.name().to_string())
                .or_insert(0) += 1;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            let lines = content.lines().count();
            if normalize_languages::support_for_path(path).is_some() {
                total_lines += lines;
            }

            let rel_path = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel_path.to_string_lossy();
            if lines >= LARGE_THRESHOLD
                && !is_lockfile(&rel_str)
                && !is_allowed(&rel_str, allow_patterns)
                && normalize_languages::support_for_path(path).is_some()
            {
                large_files.push(LargeFile {
                    path: rel_str.to_string(),
                    lines,
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
    }
}
