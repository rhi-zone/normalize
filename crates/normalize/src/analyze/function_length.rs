//! Function length analysis.
//!
//! Identifies long functions that may be candidates for refactoring.
use crate::output::OutputFormatter;
use crate::parsers;
use normalize_languages::{Language, support_for_path};
use serde::Serialize;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter;
/// Length classification for functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum LengthCategory {
    /// 1-20 lines: concise
    Short,
    /// 21-50 lines: reasonable
    Medium,
    /// 51-100 lines: getting long
    Long,
    /// 100+ lines: should be split
    TooLong,
}
impl LengthCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            LengthCategory::Short => "short",
            LengthCategory::Medium => "medium",
            LengthCategory::Long => "long",
            LengthCategory::TooLong => "too-long",
        }
    }
    pub fn as_title(&self) -> &'static str {
        match self {
            LengthCategory::Short => "Short",
            LengthCategory::Medium => "Medium",
            LengthCategory::Long => "Long",
            LengthCategory::TooLong => "Too Long",
        }
    }
}
/// Function length data.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct FunctionLength {
    pub name: String,
    pub lines: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub parent: Option<String>,
    pub file_path: Option<String>,
}
impl FunctionLength {
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
    pub fn category(&self) -> LengthCategory {
        match self.lines {
            1..=20 => LengthCategory::Short,
            21..=50 => LengthCategory::Medium,
            51..=100 => LengthCategory::Long,
            _ => LengthCategory::TooLong,
        }
    }
}
/// Length report for a file.
pub type LengthReport = super::FileReport<FunctionLength>;
impl LengthReport {
    pub fn avg_length(&self) -> f64 {
        if self.functions.is_empty() {
            0.0
        } else {
            let total: usize = self.functions.iter().map(|f| f.lines).sum();
            total as f64 / self.functions.len() as f64
        }
    }
    pub fn max_length(&self) -> usize {
        self.functions.iter().map(|f| f.lines).max().unwrap_or(0)
    }
    pub fn long_count(&self) -> usize {
        self.functions
            .iter()
            .filter(|f| f.category() == LengthCategory::Long)
            .count()
    }
    pub fn too_long_count(&self) -> usize {
        self.functions
            .iter()
            .filter(|f| f.category() == LengthCategory::TooLong)
            .count()
    }
}

impl OutputFormatter for LengthReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push("# Function Length Analysis".to_string());
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
            lines.push(format!("Average: {:.1} lines", stats.total_avg));
            lines.push(format!("Maximum: {} lines", stats.total_max));

            if stats.critical_count > 0 {
                lines.push(format!("Too Long (>100): {}", stats.critical_count));
            }
            if stats.high_count > 0 || stats.critical_count == 0 {
                lines.push(format!("Long (51-100): {}", stats.high_count));
            }
        } else {
            lines.push(format!("Functions: {}", self.functions.len()));
            lines.push(format!("Average: {:.1} lines", self.avg_length()));
            lines.push(format!("Maximum: {} lines", self.max_length()));

            let too_long = self.too_long_count();
            let long = self.long_count();
            if too_long > 0 {
                lines.push(format!("Too Long (>100): {}", too_long));
            }
            if long > 0 || too_long == 0 {
                lines.push(format!("Long (51-100): {}", long));
            }
        }

        if !self.functions.is_empty() {
            lines.push(String::new());
            lines.push("## Longest Functions".to_string());

            let mut current_cat: Option<LengthCategory> = None;
            for func in &self.functions {
                let cat = func.category();
                if Some(cat) != current_cat {
                    lines.push(format!("### {}", cat.as_title()));
                    current_cat = Some(cat);
                }
                let display_name = if let Some(ref fp) = func.file_path {
                    format!("{}:{}", fp, func.short_name())
                } else {
                    func.short_name()
                };
                lines.push(format!("{} {}", func.lines, display_name));
            }
        }

        lines.join("\n")
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::{Color, Style};

        let mut lines = Vec::new();
        lines.push(
            Style::new()
                .bold()
                .paint("Function Length Analysis")
                .to_string(),
        );
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
                "{}: {:.1} lines",
                Style::new().bold().paint("Average"),
                stats.total_avg
            ));
            lines.push(format!(
                "{}: {} lines",
                Style::new().bold().paint("Maximum"),
                stats.total_max
            ));

            if stats.critical_count > 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Red.bold().paint("Too Long (>100)"),
                    stats.critical_count
                ));
            }
            if stats.high_count > 0 || stats.critical_count == 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Yellow.bold().paint("Long (51-100)"),
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
                "{}: {:.1} lines",
                Style::new().bold().paint("Average"),
                self.avg_length()
            ));
            lines.push(format!(
                "{}: {} lines",
                Style::new().bold().paint("Maximum"),
                self.max_length()
            ));

            let too_long = self.too_long_count();
            let long = self.long_count();
            if too_long > 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Red.bold().paint("Too Long (>100)"),
                    too_long
                ));
            }
            if long > 0 || too_long == 0 {
                lines.push(format!(
                    "{}: {}",
                    Color::Yellow.bold().paint("Long (51-100)"),
                    long
                ));
            }
        }

        if !self.functions.is_empty() {
            lines.push(String::new());
            lines.push(Style::new().bold().paint("Longest Functions").to_string());

            let mut current_cat: Option<LengthCategory> = None;
            for func in &self.functions {
                let cat = func.category();
                if Some(cat) != current_cat {
                    let cat_color = match cat {
                        LengthCategory::TooLong => Color::Red,
                        LengthCategory::Long => Color::Yellow,
                        LengthCategory::Medium => Color::Blue,
                        LengthCategory::Short => Color::Green,
                    };
                    lines.push(cat_color.bold().paint(cat.as_title()).to_string());
                    current_cat = Some(cat);
                }
                let display_name = if let Some(ref fp) = func.file_path {
                    format!("{}:{}", fp, func.short_name())
                } else {
                    func.short_name()
                };
                let lines_str = match func.category() {
                    LengthCategory::TooLong => {
                        Color::Red.bold().paint(func.lines.to_string()).to_string()
                    }
                    LengthCategory::Long => Color::Yellow
                        .bold()
                        .paint(func.lines.to_string())
                        .to_string(),
                    _ => func.lines.to_string(),
                };
                lines.push(format!("{} {}", lines_str, display_name));
            }
        }

        lines.join("\n")
    }
}

pub struct LengthAnalyzer {}
impl LengthAnalyzer {
    pub fn new() -> Self {
        Self {}
    }
    pub fn analyze(&self, path: &Path, content: &str) -> LengthReport {
        let functions = match support_for_path(path) {
            Some(support) => self.analyze_with_trait(content, support),
            None => Vec::new(),
        };
        LengthReport {
            functions,
            file_path: path.to_string_lossy().to_string(),
            full_stats: None,
        }
    }
    fn analyze_with_trait(&self, content: &str, support: &dyn Language) -> Vec<FunctionLength> {
        let grammar_name = support.grammar_name();
        let tree = match parsers::parse_with_grammar(grammar_name, content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let loader = parsers::grammar_loader();

        // Prefer tags-based path when a tags query is available.
        let tags_result = loader
            .get_tags(grammar_name)
            .zip(loader.get(grammar_name))
            .and_then(|(tags_scm, ts_lang)| tree_sitter::Query::new(&ts_lang, &tags_scm).ok())
            .map(|tags_query| {
                Self::collect_functions_from_tags(&tree, &tags_query, content, support)
            });
        if let Some(result) = tags_result
            && !result.is_empty()
        {
            return result;
        }

        // Fall back to trait-based walker when function_kinds() is non-empty.
        if support.function_kinds().is_empty() {
            return Vec::new();
        }

        let mut functions = Vec::new();
        let root = tree.root_node();
        let mut cursor = root.walk();
        Self::collect_functions(&mut cursor, content, support, &mut functions, None);
        functions
    }

    /// Collect function lengths using a tags query.
    fn collect_functions_from_tags(
        tree: &tree_sitter::Tree,
        tags_query: &tree_sitter::Query,
        content: &str,
        support: &dyn Language,
    ) -> Vec<FunctionLength> {
        let capture_names = tags_query.capture_names();

        // Skip when impl-block references are used (requires trait path for nesting).
        if capture_names.contains(&"reference.implementation") {
            return Vec::new();
        }

        let root = tree.root_node();
        let mut qcursor = tree_sitter::QueryCursor::new();
        let mut matches = qcursor.matches(tags_query, root, content.as_bytes());

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

        // Sort by start line, containers first when start lines match.
        tag_nodes.sort_by(|a, b| {
            let a_start = a.node.start_position().row;
            let a_end = a.node.end_position().row;
            let b_start = b.node.start_position().row;
            let b_end = b.node.end_position().row;
            a_start.cmp(&b_start).then(b_end.cmp(&a_end))
        });

        // De-duplicate identical byte ranges.
        tag_nodes.dedup_by(|b, a| {
            a.node.start_byte() == b.node.start_byte() && a.node.end_byte() == b.node.end_byte()
        });

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

            // Find innermost enclosing container for the parent name.
            let parent_name: Option<String> = tag_nodes
                .iter()
                .enumerate()
                .filter(|(j, c)| *j != i && is_container(&c.capture_name))
                .filter(|(_, c)| {
                    let c_start = c.node.start_position().row + 1;
                    let c_end = c.node.end_position().row + 1;
                    c_start <= fn_start && c_end >= fn_end
                })
                .max_by_key(|(_, c)| c.node.start_position().row)
                .and_then(|(_, c)| support.node_name(&c.node, content))
                .map(|s| s.to_string());

            let name = match support.node_name(&tn.node, content) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let lines = fn_end.saturating_sub(fn_start) + 1;
            functions.push(FunctionLength {
                name,
                lines,
                start_line: fn_start,
                end_line: fn_end,
                parent: parent_name,
                file_path: None,
            });
        }

        functions
    }

    fn collect_functions(
        cursor: &mut tree_sitter::TreeCursor,
        content: &str,
        support: &dyn Language,
        functions: &mut Vec<FunctionLength>,
        parent: Option<&str>,
    ) {
        loop {
            let node = cursor.node();
            let kind = node.kind();
            // Check if this is a function
            if support.function_kinds().contains(&kind)
                && let Some(name) = support.node_name(&node, content)
            {
                let start_line = node.start_position().row + 1;
                let end_line = node.end_position().row + 1;
                let lines = end_line.saturating_sub(start_line) + 1;
                functions.push(FunctionLength {
                    name: name.to_string(),
                    lines,
                    start_line,
                    end_line,
                    parent: parent.map(String::from),
                    file_path: None,
                });
            }
            // Check for container (class, impl, module) holding methods
            let new_parent = if support.container_kinds().contains(&kind) {
                support.node_name(&node, content).map(|s| s.to_string())
            } else {
                parent.map(String::from)
            };
            // Recurse into children
            if cursor.goto_first_child() {
                Self::collect_functions(cursor, content, support, functions, new_parent.as_deref());
                cursor.goto_parent();
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
impl Default for LengthAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
