//! Function length analysis.
//!
//! Identifies long functions that may be candidates for refactoring.
use crate::output::{OutputFormatter, tier_color};
use crate::parsers;
use normalize_analyze::ranked::{Column, RankEntry, RiskTier, format_ranked_table};
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

    /// Map onto the shared [`RiskTier`] used by the `Risk` table column.
    pub fn tier(&self) -> RiskTier {
        match self {
            LengthCategory::Short => RiskTier::Low,
            LengthCategory::Medium => RiskTier::Moderate,
            LengthCategory::Long => RiskTier::High,
            LengthCategory::TooLong => RiskTier::Critical,
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
    /// Change in line count vs a baseline ref (set when `--diff` is used).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub delta: Option<i64>,
}

impl normalize_analyze::Entity for FunctionLength {
    fn label(&self) -> &str {
        &self.name
    }
}

impl RankEntry for FunctionLength {
    fn columns() -> Vec<Column> {
        vec![
            Column::right("Lines"),
            Column::left("Risk"),
            Column::left("Function"),
        ]
    }

    fn values(&self) -> Vec<String> {
        let lines_str = match self.delta {
            Some(d) if d > 0 => format!("{} (+{d})", self.lines),
            Some(d) if d < 0 => format!("{} ({d})", self.lines),
            Some(_) => format!("{} (±0)", self.lines),
            None => self.lines.to_string(),
        };
        let display_name = match &self.file_path {
            Some(fp) => format!("{}:{}", fp, self.short_name()),
            None => self.short_name(),
        };
        vec![
            lines_str,
            self.category().as_title().to_string(),
            display_name,
        ]
    }
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

impl LengthReport {
    /// House-style title with summary stats inline (no preamble block).
    fn title(&self) -> String {
        let (total, avg, max, too_long, long) = match &self.full_stats {
            Some(s) => (
                s.total_count,
                s.total_avg,
                s.total_max,
                s.critical_count,
                s.high_count,
            ),
            None => (
                self.functions.len(),
                self.avg_length(),
                self.max_length(),
                self.too_long_count(),
                self.long_count(),
            ),
        };
        let prefix = match &self.diff_ref {
            Some(r) => format!("# Function Length Diff vs {r}"),
            None => "# Function Length".to_string(),
        };
        format!(
            "{prefix} — {total} functions, avg {avg:.1}, max {max}, {too_long} too long, {long} long"
        )
    }

    fn empty_hint(&self) -> Option<&'static str> {
        if self.functions.is_empty() && self.full_stats.is_none() {
            Some(
                "no supported source files found — pass a file path or a directory containing code",
            )
        } else {
            None
        }
    }
}

impl OutputFormatter for LengthReport {
    fn format_text(&self) -> String {
        format_ranked_table(&self.title(), &self.functions, self.empty_hint())
    }

    fn format_pretty(&self) -> String {
        crate::output::pretty_ranked_table(
            &self.title(),
            &self.functions,
            self.empty_hint(),
            |func| Some(tier_color(func.category().tier())),
        )
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
            diff_ref: None,
        }
    }

    fn analyze_with_trait(&self, content: &str, support: &dyn Language) -> Vec<FunctionLength> {
        let grammar_name = support.grammar_name();
        let tree = match parsers::parse_with_grammar(grammar_name, content) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let loader = parsers::grammar_loader();

        loader
            .get_tags(grammar_name)
            .zip(loader.get(grammar_name).ok())
            .and_then(|(tags_scm, ts_lang)| tree_sitter::Query::new(&ts_lang, &tags_scm).ok())
            .map(|tags_query| {
                Self::collect_functions_from_tags(&tree, &tags_query, content, support)
            })
            .unwrap_or_default()
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
                delta: None,
            });
        }

        functions
    }
}

impl Default for LengthAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
