//! Tree-sitter query runner — execute a query against a file and show captures.

use normalize_languages::{parsers::grammar_loader, support_for_grammar, support_for_path};
use normalize_output::OutputFormatter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use streaming_iterator::StreamingIterator;

/// A single capture within a query match.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryCapture {
    /// Capture name (without `@` prefix).
    pub name: String,
    /// Node kind.
    pub kind: String,
    /// Start line (1-based).
    pub start_line: usize,
    /// Start column (0-based).
    pub start_col: usize,
    /// End line (1-based).
    pub end_line: usize,
    /// End column (0-based).
    pub end_col: usize,
    /// Source text of the captured node (truncated at 200 chars).
    pub text: String,
}

/// One pattern match, containing its captures.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryMatch {
    /// 0-based match index.
    pub index: usize,
    /// Captures in this match.
    pub captures: Vec<QueryCapture>,
}

/// Report returned by `normalize analyze query`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QueryReport {
    /// File that was queried.
    pub file: String,
    /// Detected or overridden language name.
    pub language: String,
    /// Query source text (or file path).
    pub query_source: String,
    /// Total number of matches.
    pub match_count: usize,
    /// All matches.
    pub matches: Vec<QueryMatch>,
}

impl OutputFormatter for QueryReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "File: {}  Language: {}  Matches: {}\n",
            self.file, self.language, self.match_count
        ));
        if self.matches.is_empty() {
            out.push_str("No matches.\n");
            return out;
        }
        for m in &self.matches {
            out.push_str(&format!(
                "\nMatch {} ({} capture{}):\n",
                m.index + 1,
                m.captures.len(),
                if m.captures.len() == 1 { "" } else { "s" }
            ));
            for cap in &m.captures {
                out.push_str(&format!(
                    "  @{}: {} {:?} [{}:{}-{}:{}]\n",
                    cap.name,
                    cap.kind,
                    cap.text,
                    cap.start_line,
                    cap.start_col,
                    cap.end_line,
                    cap.end_col
                ));
            }
        }
        out
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;
        let mut out = String::new();
        out.push_str(&format!(
            "File: {}  Language: {}  Matches: {}\n",
            Color::Cyan.bold().paint(&self.file),
            Color::Yellow.paint(&self.language),
            Color::Green.bold().paint(self.match_count.to_string())
        ));
        if self.matches.is_empty() {
            out.push_str("No matches.\n");
            return out;
        }
        for m in &self.matches {
            out.push_str(&format!(
                "\n{} ({} capture{}):\n",
                Color::Cyan.bold().paint(format!("Match {}", m.index + 1)),
                m.captures.len(),
                if m.captures.len() == 1 { "" } else { "s" }
            ));
            for cap in &m.captures {
                let loc = format!(
                    "[{}:{}-{}:{}]",
                    cap.start_line, cap.start_col, cap.end_line, cap.end_col
                );
                out.push_str(&format!(
                    "  {}: {} {} {}\n",
                    Color::Yellow.paint(format!("@{}", cap.name)),
                    Color::Cyan.paint(&cap.kind),
                    Color::Green.paint(format!("{:?}", cap.text)),
                    loc
                ));
            }
        }
        out
    }
}

/// Run a tree-sitter query against a file and return the matches.
///
/// - `query_str`: either an inline s-expression query, or a path ending in `.scm`.
/// - `language_override`: if supplied, use this grammar name instead of detecting from extension.
pub fn query_file(
    file: &Path,
    query_str: &str,
    language_override: Option<&str>,
) -> Result<QueryReport, String> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| format!("could not read '{}': {}", file.display(), e))?;

    // Detect language.
    let lang_name: String = if let Some(lang) = language_override {
        lang.to_string()
    } else {
        support_for_path(file)
            .map(|l| l.grammar_name().to_string())
            .ok_or_else(|| {
                format!(
                    "unsupported file type: .{}",
                    file.extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("<unknown>")
                )
            })?
    };

    // Load grammar.
    let loader = grammar_loader();
    let ts_lang = loader
        .get(&lang_name)
        .or_else(|| support_for_grammar(&lang_name).and_then(|s| loader.get(s.grammar_name())))
        .ok_or_else(|| format!("grammar not loaded for language '{lang_name}'"))?;

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&ts_lang)
        .map_err(|e| format!("failed to set language: {e}"))?;

    let tree = parser
        .parse(&content, None)
        .ok_or_else(|| "tree-sitter parse failed".to_string())?;

    // Load query source (file or inline).
    let (query_source, query_text) = if query_str.ends_with(".scm") && Path::new(query_str).exists()
    {
        let text = std::fs::read_to_string(query_str)
            .map_err(|e| format!("could not read query file '{query_str}': {e}"))?;
        (query_str.to_string(), text)
    } else {
        (query_str.to_string(), query_str.to_string())
    };

    let query = tree_sitter::Query::new(&ts_lang, &query_text)
        .map_err(|e| format!("invalid query: {e}"))?;

    let capture_names = query.capture_names().to_vec();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut iter = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut matches: Vec<QueryMatch> = Vec::new();
    let mut index = 0usize;
    while let Some(m) = iter.next() {
        let captures = m
            .captures
            .iter()
            .map(|cap| {
                let node = cap.node;
                let start = node.start_position();
                let end = node.end_position();
                let raw_text = node.utf8_text(content.as_bytes()).unwrap_or("");
                let text = if raw_text.len() > 200 {
                    format!("{}…", &raw_text[..200])
                } else {
                    raw_text.to_string()
                };
                QueryCapture {
                    name: capture_names
                        .get(cap.index as usize)
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    kind: node.kind().to_string(),
                    start_line: start.row + 1,
                    start_col: start.column,
                    end_line: end.row + 1,
                    end_col: end.column,
                    text,
                }
            })
            .collect();
        matches.push(QueryMatch { index, captures });
        index += 1;
    }

    let match_count = matches.len();
    Ok(QueryReport {
        file: file.display().to_string(),
        language: lang_name,
        query_source,
        match_count,
        matches,
    })
}
