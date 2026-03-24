//! Line budget analysis: classify every line in the project by purpose.
//!
//! Answers "where do our lines go?" — business logic, tests, docs, config, generated, vendored.

use crate::commands::analyze::test_ratio::{
    discover_module_dirs, module_key, split_rust_test_lines,
};
use crate::output::OutputFormatter;
use normalize_analyze::ranked::{
    Column, DiffableRankEntry, RankEntry, format_delta, format_ranked_table,
};
use normalize_languages::is_test_path;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

/// Primary purpose category for a file (or portion of a file).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BudgetCategory {
    BusinessLogic,
    Documentation,
    TestCode,
    Generated,
    ConfigBuild,
    Vendored,
}

impl fmt::Display for BudgetCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BusinessLogic => write!(f, "business logic"),
            Self::Documentation => write!(f, "documentation"),
            Self::TestCode => write!(f, "test code"),
            Self::Generated => write!(f, "generated"),
            Self::ConfigBuild => write!(f, "config/build"),
            Self::Vendored => write!(f, "vendored"),
        }
    }
}

/// Line count for a single category.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct CategoryEntry {
    pub category: BudgetCategory,
    pub lines: usize,
    pub pct: f64,
}

/// Per-module budget breakdown.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ModuleBudget {
    pub module: String,
    pub total_lines: usize,
    pub logic_pct: f64,
    pub test_pct: f64,
    pub other_pct: f64,
    /// Delta vs baseline (set by `--diff`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
}

impl RankEntry for CategoryEntry {
    fn columns() -> Vec<Column> {
        vec![
            Column::left("Category"),
            Column::right("Lines"),
            Column::right("Pct"),
        ]
    }

    fn values(&self) -> Vec<String> {
        vec![
            self.category.to_string(),
            format_num(self.lines),
            format!("{:.1}%", self.pct),
        ]
    }
}

impl RankEntry for ModuleBudget {
    fn columns() -> Vec<Column> {
        vec![
            Column::left("Module"),
            Column::right("Lines"),
            Column::right("Logic"),
            Column::right("Test"),
            Column::right("Other"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let lines_str = match self.delta {
            Some(d) => format!(
                "{:.0}K ({})",
                self.total_lines as f64 / 1000.0,
                format_delta(d, false)
            ),
            None => format!("{:.0}K", self.total_lines as f64 / 1000.0),
        };
        vec![
            self.module.clone(),
            lines_str,
            format!("{:.0}%", self.logic_pct),
            format!("{:.0}%", self.test_pct),
            format!("{:.0}%", self.other_pct),
        ]
    }
}

impl DiffableRankEntry for ModuleBudget {
    fn diff_key(&self) -> &str {
        &self.module
    }
    fn diff_score(&self) -> f64 {
        self.total_lines as f64
    }
    fn set_delta(&mut self, delta: Option<f64>) {
        self.delta = delta;
    }
    fn delta(&self) -> Option<f64> {
        self.delta
    }
}

/// Report returned by `analyze budget`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct LineBudgetReport {
    pub root: String,
    pub total_lines: usize,
    pub categories: Vec<CategoryEntry>,
    pub modules: Vec<ModuleBudget>,
    /// Set when `--diff` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_ref: Option<String>,
}

impl OutputFormatter for LineBudgetReport {
    fn format_text(&self) -> String {
        let total_k = self.total_lines as f64 / 1000.0;
        let mut out = format_ranked_table(
            &format!("# Line Budget: {} ({:.0}K lines)", self.root, total_k),
            &self.categories,
            None,
        );

        if !self.modules.is_empty() {
            out.push_str("\n\n");
            out.push_str(&format_ranked_table("## By Module", &self.modules, None));
        }

        out
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;

        let mut out = Vec::new();

        let total_k = self.total_lines as f64 / 1000.0;
        out.push(format!(
            "{}",
            Color::White.bold().paint(format!(
                "Line Budget: {} ({:.0}K lines)",
                self.root, total_k
            )),
        ));
        out.push(String::new());
        out.push(format!(
            "  {:<18} {:>8}  {:>6}  {}",
            Color::DarkGray.paint("category"),
            Color::DarkGray.paint("lines"),
            Color::DarkGray.paint("pct"),
            Color::DarkGray.paint("bar"),
        ));
        out.push(format!("  {}", Color::DarkGray.paint("-".repeat(60))));

        let bar_width = 25;
        for entry in &self.categories {
            let filled = ((entry.pct / 100.0) * bar_width as f64).round() as usize;
            let bar_str = format!(
                "{}{}",
                "#".repeat(filled.min(bar_width)),
                " ".repeat(bar_width.saturating_sub(filled)),
            );
            let color = category_color(entry.category);
            out.push(format!(
                "  {:<18} {:>8}  {:>5.1}%  {}",
                color.paint(entry.category.to_string()),
                format_num(entry.lines),
                entry.pct,
                color.paint(bar_str),
            ));
        }

        out.push(format!("  {}", Color::DarkGray.paint("-".repeat(60))));
        out.push(format!(
            "  {:<18} {:>8}  {:>5.1}%",
            "total",
            format_num(self.total_lines),
            100.0,
        ));

        if !self.modules.is_empty() {
            out.push(String::new());
            out.push(format!("{}", Color::White.bold().paint("By Module"),));
            for m in &self.modules {
                out.push(format!(
                    "  {:<40} {:>5.0}K  ({:.0}% logic, {:.0}% test, {:.0}% other)",
                    truncate_path(&m.module, 40),
                    m.total_lines as f64 / 1000.0,
                    m.logic_pct,
                    m.test_pct,
                    m.other_pct,
                ));
            }
        }

        out.join("\n")
    }
}

fn category_color(cat: BudgetCategory) -> nu_ansi_term::Color {
    use nu_ansi_term::Color;
    match cat {
        BudgetCategory::BusinessLogic => Color::Green,
        BudgetCategory::Documentation => Color::Blue,
        BudgetCategory::TestCode => Color::Cyan,
        BudgetCategory::Generated => Color::Yellow,
        BudgetCategory::ConfigBuild => Color::Purple,
        BudgetCategory::Vendored => Color::Red,
    }
}

fn format_num(n: usize) -> String {
    if n >= 1_000_000 {
        format!(
            "{},{:03},{:03}",
            n / 1_000_000,
            (n / 1_000) % 1_000,
            n % 1_000
        )
    } else if n >= 1_000 {
        format!("{},{:03}", n / 1_000, n % 1_000)
    } else {
        format!("{}", n)
    }
}

fn truncate_path(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - (max - 1)..])
    }
}

/// Generated-file marker strings (checked in first 5 lines).
const GENERATED_MARKERS: &[&str] = &[
    "@generated",
    "DO NOT EDIT",
    "Code generated by",
    "auto-generated",
    "automatically generated",
];

/// Generated-file path patterns.
fn is_generated_path(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.contains(".gen.")
        || name.contains(".generated.")
        || name.ends_with(".pb.go")
        || name.ends_with(".pb.rs")
        || name.contains("_generated.")
        || rel.starts_with("generated/")
        || rel.contains("/generated/")
}

fn is_generated_content(content: &str) -> bool {
    for line in content.lines().take(5) {
        let lower = line.to_lowercase();
        for marker in GENERATED_MARKERS {
            if lower.contains(&marker.to_lowercase()) {
                return true;
            }
        }
    }
    false
}

fn is_vendored_path(rel: &str) -> bool {
    let p = rel.to_lowercase();
    p.starts_with("vendor/")
        || p.starts_with("vendored/")
        || p.starts_with("third_party/")
        || p.starts_with("third-party/")
        || p.contains("/vendor/")
        || p.contains("/vendored/")
        || p.contains("/third_party/")
        || p.contains("/third-party/")
}

fn is_doc_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let lower = name.to_lowercase();
    lower.ends_with(".md")
        || lower.ends_with(".rst")
        || lower.ends_with(".txt")
        || lower.starts_with("license")
        || lower.starts_with("changelog")
}

fn is_config_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let lower = name.to_lowercase();
    let p = rel.to_lowercase();
    lower.ends_with(".toml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".json")
        || lower.ends_with(".nix")
        || lower.ends_with(".lock")
        || lower == "makefile"
        || lower.ends_with(".cmake")
        || lower == "justfile"
        || lower.starts_with("dockerfile")
        || p.starts_with(".github/")
        || p.contains("/.github/")
        || p.starts_with(".normalize/")
        || p.contains("/.normalize/")
}

/// Per-file classification result.
struct FileClassification {
    rel_path: String,
    logic_lines: usize,
    test_lines: usize,
    doc_lines: usize,
    config_lines: usize,
    generated_lines: usize,
    vendored_lines: usize,
}

fn classify_file(rel_path: &str, content: &str) -> FileClassification {
    let total_lines = content.lines().count().max(1);

    // Priority 1: vendored
    if is_vendored_path(rel_path) {
        return FileClassification {
            rel_path: rel_path.to_string(),
            logic_lines: 0,
            test_lines: 0,
            doc_lines: 0,
            config_lines: 0,
            generated_lines: 0,
            vendored_lines: total_lines,
        };
    }

    // Priority 2: generated (path or content)
    if is_generated_path(rel_path) || is_generated_content(content) {
        return FileClassification {
            rel_path: rel_path.to_string(),
            logic_lines: 0,
            test_lines: 0,
            doc_lines: 0,
            config_lines: 0,
            generated_lines: total_lines,
            vendored_lines: 0,
        };
    }

    // Priority 3: docs
    if is_doc_file(rel_path) {
        return FileClassification {
            rel_path: rel_path.to_string(),
            logic_lines: 0,
            test_lines: 0,
            doc_lines: total_lines,
            config_lines: 0,
            generated_lines: 0,
            vendored_lines: 0,
        };
    }

    // Priority 4: config/build
    if is_config_file(rel_path) {
        return FileClassification {
            rel_path: rel_path.to_string(),
            logic_lines: 0,
            test_lines: 0,
            doc_lines: 0,
            config_lines: total_lines,
            generated_lines: 0,
            vendored_lines: 0,
        };
    }

    // Priority 5: test file (entire file is test)
    if is_test_path(Path::new(rel_path)) {
        return FileClassification {
            rel_path: rel_path.to_string(),
            logic_lines: 0,
            test_lines: total_lines,
            doc_lines: 0,
            config_lines: 0,
            generated_lines: 0,
            vendored_lines: 0,
        };
    }

    // Priority 6: source file — split Rust files by #[cfg(test)]
    let ext = Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext == "rs" {
        let lc = split_rust_test_lines(content);
        return FileClassification {
            rel_path: rel_path.to_string(),
            logic_lines: lc.impl_lines,
            test_lines: lc.test_lines,
            doc_lines: 0,
            config_lines: 0,
            generated_lines: 0,
            vendored_lines: 0,
        };
    }

    // Everything else: business logic
    FileClassification {
        rel_path: rel_path.to_string(),
        logic_lines: total_lines,
        test_lines: 0,
        doc_lines: 0,
        config_lines: 0,
        generated_lines: 0,
        vendored_lines: 0,
    }
}

/// Analyze line budget across the entire codebase.
pub fn analyze_budget(root: &Path, module_limit: usize) -> LineBudgetReport {
    let module_dirs = discover_module_dirs(root);
    let all_files = crate::path_resolve::all_files(root);

    let classifications: Vec<FileClassification> = all_files
        .par_iter()
        .filter(|f| f.kind == normalize_path_resolve::PathMatchKind::File)
        .filter_map(|f| {
            let abs_path = root.join(&f.path);
            let content = std::fs::read_to_string(&abs_path).ok()?;
            Some(classify_file(&f.path, &content))
        })
        .collect();

    // Aggregate totals by category
    let mut totals: BTreeMap<BudgetCategory, usize> = BTreeMap::new();
    for cat in &[
        BudgetCategory::BusinessLogic,
        BudgetCategory::Documentation,
        BudgetCategory::TestCode,
        BudgetCategory::Generated,
        BudgetCategory::ConfigBuild,
        BudgetCategory::Vendored,
    ] {
        totals.insert(*cat, 0);
    }

    for c in &classifications {
        *totals.entry(BudgetCategory::BusinessLogic).or_default() += c.logic_lines;
        *totals.entry(BudgetCategory::TestCode).or_default() += c.test_lines;
        *totals.entry(BudgetCategory::Documentation).or_default() += c.doc_lines;
        *totals.entry(BudgetCategory::ConfigBuild).or_default() += c.config_lines;
        *totals.entry(BudgetCategory::Generated).or_default() += c.generated_lines;
        *totals.entry(BudgetCategory::Vendored).or_default() += c.vendored_lines;
    }

    let total_lines: usize = totals.values().sum();

    // Build category entries in display order
    let display_order = [
        BudgetCategory::BusinessLogic,
        BudgetCategory::Documentation,
        BudgetCategory::TestCode,
        BudgetCategory::Generated,
        BudgetCategory::ConfigBuild,
        BudgetCategory::Vendored,
    ];
    let categories: Vec<CategoryEntry> = display_order
        .iter()
        .map(|cat| {
            let lines = totals[cat];
            CategoryEntry {
                category: *cat,
                lines,
                pct: if total_lines > 0 {
                    lines as f64 / total_lines as f64 * 100.0
                } else {
                    0.0
                },
            }
        })
        .collect();

    // Group by module
    let mut module_data: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new(); // (logic, test, other)
    for c in &classifications {
        let key = module_key(&c.rel_path, &module_dirs);
        let entry = module_data.entry(key).or_default();
        entry.0 += c.logic_lines;
        entry.1 += c.test_lines;
        entry.2 += c.doc_lines + c.config_lines + c.generated_lines + c.vendored_lines;
    }

    let mut modules: Vec<ModuleBudget> = module_data
        .into_iter()
        .filter(|(_, (l, t, o))| l + t + o > 0)
        .map(|(module, (logic, test, other))| {
            let total = logic + test + other;
            ModuleBudget {
                module,
                total_lines: total,
                logic_pct: if total > 0 {
                    logic as f64 / total as f64 * 100.0
                } else {
                    0.0
                },
                test_pct: if total > 0 {
                    test as f64 / total as f64 * 100.0
                } else {
                    0.0
                },
                other_pct: if total > 0 {
                    other as f64 / total as f64 * 100.0
                } else {
                    0.0
                },
                delta: None,
            }
        })
        .collect();

    // Sort by total lines descending
    modules.sort_by(|a, b| b.total_lines.cmp(&a.total_lines));
    modules.truncate(module_limit);

    LineBudgetReport {
        root: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        total_lines,
        categories,
        modules,
        diff_ref: None,
    }
}
