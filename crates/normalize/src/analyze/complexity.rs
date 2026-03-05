//! Cyclomatic complexity analysis.
//!
//! Calculates McCabe cyclomatic complexity for functions.
//! Complexity = number of decision points + 1

use crate::output::OutputFormatter;
use crate::parsers;
use normalize_facts::extract::compute_complexity;
use normalize_languages::{Language, support_for_path};
use serde::Serialize;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter;

/// Risk classification based on McCabe cyclomatic complexity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum RiskLevel {
    /// 1-5: Simple, easy to test
    Low,
    /// 6-10: Manageable, may need review
    Moderate,
    /// 11-20: Complex, harder to test and maintain
    High,
    /// 21+: Should be refactored, often untestable
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Moderate => "moderate",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }

    pub fn as_title(&self) -> &'static str {
        match self {
            RiskLevel::Low => "Low",
            RiskLevel::Moderate => "Moderate",
            RiskLevel::High => "High",
            RiskLevel::Critical => "Critical",
        }
    }
}

/// Complexity data for a function
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FunctionComplexity {
    pub name: String,
    pub complexity: usize,
    pub start_line: usize,
    #[allow(dead_code)] // Part of public API, may be used by consumers
    pub end_line: usize,
    pub parent: Option<String>,    // class/struct name for methods
    pub file_path: Option<String>, // file path for codebase-wide reports
}

impl normalize_analyze::Entity for FunctionComplexity {
    fn label(&self) -> &str {
        &self.name
    }
}

impl FunctionComplexity {
    pub fn qualified_name(&self) -> String {
        let base = if let Some(parent) = &self.parent {
            format!("{}.{}", parent, self.name)
        } else {
            self.name.clone()
        };
        if let Some(fp) = &self.file_path {
            format!("{}:{}", fp, base)
        } else {
            base
        }
    }

    pub fn short_name(&self) -> String {
        if let Some(parent) = &self.parent {
            format!("{}.{}", parent, self.name)
        } else {
            self.name.clone()
        }
    }

    /// Line count of the function
    pub fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Risk classification based on McCabe cyclomatic complexity thresholds.
    ///
    /// Industry-standard ranges (similar to SonarQube, Code Climate):
    /// - 1-5: Low risk - simple, easy to test
    /// - 6-10: Moderate risk - still manageable, may need review
    /// - 11-20: High risk - complex, harder to test and maintain
    /// - 21+: Very high risk - should be refactored, often untestable
    ///
    /// McCabe's original paper (1976) suggested 10 as the upper limit.
    pub fn risk_level(&self) -> RiskLevel {
        match self.complexity {
            1..=5 => RiskLevel::Low,
            6..=10 => RiskLevel::Moderate,
            11..=20 => RiskLevel::High,
            _ => RiskLevel::Critical,
        }
    }
}

/// Complexity report for a file
pub type ComplexityReport = super::FileReport<FunctionComplexity>;

impl ComplexityReport {
    pub fn total_complexity(&self) -> usize {
        self.functions.iter().map(|f| f.complexity).sum()
    }

    pub fn avg_complexity(&self) -> f64 {
        if self.functions.is_empty() {
            0.0
        } else {
            let total: usize = self.functions.iter().map(|f| f.complexity).sum();
            total as f64 / self.functions.len() as f64
        }
    }

    pub fn max_complexity(&self) -> usize {
        self.functions
            .iter()
            .map(|f| f.complexity)
            .max()
            .unwrap_or(0)
    }

    /// Count of high risk functions (complexity 11-20)
    pub fn high_risk_count(&self) -> usize {
        self.functions
            .iter()
            .filter(|f| f.risk_level() == RiskLevel::High)
            .count()
    }

    /// Count of critical risk functions (complexity 21+)
    pub fn critical_risk_count(&self) -> usize {
        self.functions
            .iter()
            .filter(|f| f.risk_level() == RiskLevel::Critical)
            .count()
    }

    /// Calculate complexity score (0-100).
    /// 100 if no high-risk functions, decreases with complex code.
    pub fn score(&self) -> f64 {
        let high_risk = self.high_risk_count();
        let total = self.functions.len();
        if total == 0 {
            return 100.0;
        }
        let ratio = high_risk as f64 / total as f64;
        (100.0 * (1.0 - ratio)).max(0.0)
    }
}

impl OutputFormatter for ComplexityReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("# Complexity Analysis".to_string());
        lines.push(String::new());

        if let Some(ref stats) = self.full_stats {
            let shown = self.functions.len();
            if stats.total_count > shown {
                lines.push(format!(
                    "Functions: {} (showing {})",
                    stats.total_count, shown
                ));
            } else {
                lines.push(format!("Functions: {}", stats.total_count));
            }
            lines.push(format!("Average: {:.1}", stats.total_avg));
            lines.push(format!("Maximum: {}", stats.total_max));

            if stats.critical_count > 0 {
                lines.push(format!("Critical (>20): {}", stats.critical_count));
            }
            if stats.high_count > 0 || stats.critical_count == 0 {
                lines.push(format!("High risk (11-20): {}", stats.high_count));
            }
        } else {
            lines.push(format!("Functions: {}", self.functions.len()));
            lines.push(format!("Average: {:.1}", self.avg_complexity()));
            lines.push(format!("Maximum: {}", self.max_complexity()));

            let crit = self.critical_risk_count();
            let high = self.high_risk_count();
            if crit > 0 {
                lines.push(format!("Critical (>20): {}", crit));
            }
            if high > 0 || crit == 0 {
                lines.push(format!("High risk (11-20): {}", high));
            }
        }

        if !self.functions.is_empty() {
            lines.push(String::new());
            lines.push("## Complex Functions".to_string());

            let mut current_risk: Option<RiskLevel> = None;
            for func in &self.functions {
                let risk = func.risk_level();
                if Some(risk) != current_risk {
                    lines.push(format!("### {}", risk.as_title()));
                    current_risk = Some(risk);
                }
                let display_name = if let Some(ref fp) = func.file_path {
                    format!("{}:{}", fp, func.short_name())
                } else {
                    func.short_name()
                };
                lines.push(format!("{} {}", func.complexity, display_name));
            }
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::{Color, Style};

        let mut lines = Vec::new();
        lines.push(Style::new().bold().paint("Complexity Analysis").to_string());
        lines.push(String::new());

        if let Some(ref stats) = self.full_stats {
            let shown = self.functions.len();
            if stats.total_count > shown {
                lines.push(format!(
                    "{}: {} (showing {})",
                    Style::new().bold().paint("Functions"),
                    stats.total_count,
                    shown
                ));
            } else {
                lines.push(format!(
                    "{}: {}",
                    Style::new().bold().paint("Functions"),
                    stats.total_count
                ));
            }
            lines.push(format!(
                "{}: {:.1}",
                Style::new().bold().paint("Average"),
                stats.total_avg
            ));
            lines.push(format!(
                "{}: {}",
                Style::new().bold().paint("Maximum"),
                stats.total_max
            ));

            if stats.critical_count > 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Red.bold().paint("Critical (>20)"),
                    stats.critical_count
                ));
            }
            if stats.high_count > 0 || stats.critical_count == 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Yellow.bold().paint("High risk (11-20)"),
                    stats.high_count
                ));
            }
        } else {
            lines.push(format!(
                "{}: {}",
                Style::new().bold().paint("Functions"),
                self.functions.len()
            ));
            lines.push(format!(
                "{}: {:.1}",
                Style::new().bold().paint("Average"),
                self.avg_complexity()
            ));
            lines.push(format!(
                "{}: {}",
                Style::new().bold().paint("Maximum"),
                self.max_complexity()
            ));

            let crit = self.critical_risk_count();
            let high = self.high_risk_count();
            if crit > 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Red.bold().paint("Critical (>20)"),
                    crit
                ));
            }
            if high > 0 || crit == 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Yellow.bold().paint("High risk (11-20)"),
                    high
                ));
            }
        }

        if !self.functions.is_empty() {
            lines.push(String::new());
            lines.push(Style::new().bold().paint("Complex Functions").to_string());

            let mut current_risk: Option<RiskLevel> = None;
            for func in &self.functions {
                let risk = func.risk_level();
                if Some(risk) != current_risk {
                    let risk_color = match risk {
                        RiskLevel::Critical => Color::Red,
                        RiskLevel::High => Color::Yellow,
                        RiskLevel::Moderate => Color::Blue,
                        RiskLevel::Low => Color::Green,
                    };
                    lines.push(risk_color.bold().paint(risk.as_title()).to_string());
                    current_risk = Some(risk);
                }
                let display_name = if let Some(ref fp) = func.file_path {
                    format!("{}:{}", fp, func.short_name())
                } else {
                    func.short_name()
                };
                let complexity_str = match func.risk_level() {
                    RiskLevel::Critical => Color::Red
                        .bold()
                        .paint(func.complexity.to_string())
                        .to_string(),
                    RiskLevel::High => Color::Yellow
                        .bold()
                        .paint(func.complexity.to_string())
                        .to_string(),
                    _ => func.complexity.to_string(),
                };
                lines.push(format!("{} {}", complexity_str, display_name));
            }
        }

        lines.join("\n")
    }
}

pub struct ComplexityAnalyzer {}

impl Default for ComplexityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexityAnalyzer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn analyze(&self, path: &Path, content: &str) -> ComplexityReport {
        let functions = match support_for_path(path) {
            Some(support) => self.analyze_with_trait(content, support),
            None => Vec::new(),
        };

        ComplexityReport {
            functions,
            file_path: path.to_string_lossy().to_string(),
            full_stats: None,
        }
    }

    /// Analyze using the Language trait with tags.scm.
    ///
    /// Uses the tags query to identify function/method nodes and compute
    /// per-function complexity (via `.complexity.scm` when present).
    fn analyze_with_trait(&self, content: &str, support: &dyn Language) -> Vec<FunctionComplexity> {
        let grammar_name = support.grammar_name();
        let tree = match parsers::parse_with_grammar(grammar_name, content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let loader = parsers::grammar_loader();

        loader
            .get_tags(grammar_name)
            .zip(loader.get(grammar_name))
            .and_then(|(tags_scm, ts_lang)| tree_sitter::Query::new(&ts_lang, &tags_scm).ok())
            .map(|tags_query| {
                self.collect_functions_from_tags(
                    &tree,
                    &tags_query,
                    content,
                    support,
                    loader.as_ref(),
                    grammar_name,
                )
            })
            .unwrap_or_default()
    }

    /// Collect function complexity data using a tags query.
    ///
    /// Runs the tags query to find `@definition.function` and `@definition.method`
    /// nodes, computes complexity for each using the complexity query (if any),
    /// and reconstructs parent names via line-range containment.
    fn collect_functions_from_tags(
        &self,
        tree: &tree_sitter::Tree,
        tags_query: &tree_sitter::Query,
        content: &str,
        support: &dyn Language,
        loader: &normalize_languages::GrammarLoader,
        grammar_name: &str,
    ) -> Vec<FunctionComplexity> {
        use streaming_iterator::StreamingIterator;

        let capture_names = tags_query.capture_names();

        let complexity_query = loader.get_complexity(grammar_name).and_then(|scm| {
            let grammar = loader.get(grammar_name)?;
            tree_sitter::Query::new(&grammar, &scm).ok()
        });

        let root = tree.root_node();
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(tags_query, root, content.as_bytes());

        // Collect (node, capture_name) for all function/method/container definitions.
        struct TagNode<'t> {
            node: tree_sitter::Node<'t>,
            capture_name: String,
        }

        let mut tag_nodes: Vec<TagNode<'_>> = Vec::new();

        while let Some(m) = matches.next() {
            for capture in m.captures {
                let cn = capture_names[capture.index as usize];
                if matches!(
                    cn,
                    "definition.function"
                        | "definition.method"
                        | "definition.class"
                        | "definition.module"
                        | "definition.interface"
                ) {
                    tag_nodes.push(TagNode {
                        node: capture.node,
                        capture_name: cn.to_string(),
                    });
                }
            }
        }

        if tag_nodes.is_empty() {
            return Vec::new();
        }

        // Sort by start line, containers first when lines match.
        tag_nodes.sort_by(|a, b| {
            let a_start = a.node.start_position().row;
            let a_end = a.node.end_position().row;
            let b_start = b.node.start_position().row;
            let b_end = b.node.end_position().row;
            a_start.cmp(&b_start).then(b_end.cmp(&a_end))
        });

        // De-duplicate identical byte ranges (some queries match the same node twice).
        tag_nodes.dedup_by(|b, a| {
            a.node.start_byte() == b.node.start_byte() && a.node.end_byte() == b.node.end_byte()
        });

        // For each function/method node, find its enclosing container name.
        let is_container = |cn: &str| {
            matches!(
                cn,
                "definition.class" | "definition.module" | "definition.interface"
            )
        };

        let mut functions = Vec::new();

        for i in 0..tag_nodes.len() {
            let tn = &tag_nodes[i];
            if is_container(&tn.capture_name) {
                continue;
            }

            let fn_start = tn.node.start_position().row + 1;
            let fn_end = tn.node.end_position().row + 1;

            // Find innermost enclosing container.
            let parent_name: Option<String> = tag_nodes
                .iter()
                .enumerate()
                .filter(|(j, c)| *j != i && is_container(&c.capture_name))
                .filter(|(_, c)| {
                    let c_start = c.node.start_position().row + 1;
                    let c_end = c.node.end_position().row + 1;
                    c_start <= fn_start && c_end >= fn_end
                })
                // innermost = largest start_line among enclosing containers
                .max_by_key(|(_, c)| c.node.start_position().row)
                .and_then(|(_, c)| support.node_name(&c.node, content))
                .map(|s| s.to_string());

            let name = match support.node_name(&tn.node, content) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let complexity = if let Some(ref cq) = complexity_query {
                self.count_complexity_with_query(&tn.node, cq, content)
            } else {
                compute_complexity(&tn.node, support, content.as_bytes())
            };

            functions.push(FunctionComplexity {
                name,
                complexity,
                start_line: fn_start,
                end_line: fn_end,
                parent: parent_name,
                file_path: None,
            });
        }

        functions
    }

    /// Count complexity using a tree-sitter query with `@complexity` captures.
    /// Returns base complexity (1) + number of `@complexity` matches within the node.
    fn count_complexity_with_query(
        &self,
        node: &tree_sitter::Node,
        query: &tree_sitter::Query,
        content: &str,
    ) -> usize {
        let complexity_idx = query
            .capture_names()
            .iter()
            .position(|n| *n == "complexity");

        let Some(complexity_idx) = complexity_idx else {
            return 1; // No @complexity capture in query
        };

        let mut qcursor = tree_sitter::QueryCursor::new();
        // Restrict to this function's byte range
        qcursor.set_byte_range(node.byte_range());

        let mut complexity = 1usize; // Base complexity
        let mut matches = qcursor.matches(query, *node, content.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                if capture.index as usize == complexity_idx {
                    complexity += 1;
                }
            }
        }
        complexity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_python_complexity() {
        let analyzer = ComplexityAnalyzer::new();
        let content = r#"
def simple():
    return 1

def with_if(x):
    if x > 0:
        return x
    else:
        return -x

def with_loop(items):
    total = 0
    for item in items:
        if item > 0:
            total += item
    return total
"#;
        let report = analyzer.analyze(&PathBuf::from("test.py"), content);

        let simple = report
            .functions
            .iter()
            .find(|f| f.name == "simple")
            .unwrap();
        assert_eq!(simple.complexity, 1);

        let with_if = report
            .functions
            .iter()
            .find(|f| f.name == "with_if")
            .unwrap();
        assert_eq!(with_if.complexity, 2); // 1 base + 1 if

        let with_loop = report
            .functions
            .iter()
            .find(|f| f.name == "with_loop")
            .unwrap();
        assert_eq!(with_loop.complexity, 3); // 1 base + 1 for + 1 if
    }

    #[test]
    fn test_rust_complexity() {
        let analyzer = ComplexityAnalyzer::new();
        let content = r#"
fn simple() -> i32 {
    1
}

fn with_if(x: i32) -> i32 {
    if x > 0 {
        x
    } else {
        -x
    }
}

fn with_match(x: Option<i32>) -> i32 {
    match x {
        Some(v) => v,
        None => 0,
    }
}
"#;
        let report = analyzer.analyze(&PathBuf::from("test.rs"), content);

        let simple = report
            .functions
            .iter()
            .find(|f| f.name == "simple")
            .unwrap();
        assert_eq!(simple.complexity, 1);

        let with_if = report
            .functions
            .iter()
            .find(|f| f.name == "with_if")
            .unwrap();
        assert!(
            with_if.complexity >= 2,
            "with_if should have complexity >= 2, got {}",
            with_if.complexity
        );

        let with_match = report
            .functions
            .iter()
            .find(|f| f.name == "with_match")
            .unwrap();
        assert!(
            with_match.complexity >= 1,
            "with_match should have complexity >= 1, got {}",
            with_match.complexity
        );
    }
}
