//! Context window construction from the structural index.
//!
//! Each chunk is a rich text window centered on a symbol:
//!
//!   symbol name + signature
//!   + doc comment (tree-sitter extracted)
//!   + parent module/crate path
//!   + callers (top N by frequency)
//!   + callees
//!   + co-change neighbors
//!
//! Additional source types:
//!
//! - **`doc`**: Markdown files chunked by heading section. Each section becomes
//!   one chunk with a breadcrumb of parent headings prepended for context.
//! - **`commit`**: Git commit messages (subject + body), keyed by commit hash.
//!
//! The quality of the embedding is directly upstream of the quality of the
//! index. Better extraction -> better context windows -> better embeddings.

/// A chunk ready for embedding. Each chunk corresponds to one row in the
/// `embeddings` table.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Source type tag: "symbol", "doc", "commit", or "cluster".
    pub source_type: String,
    /// Relative file path containing the source.
    pub source_path: String,
    /// FK into the `symbols` table (rowid) if this is a symbol chunk.
    pub source_id: Option<i64>,
    /// The text passed to the embedding model.
    pub text: String,
    /// Git HEAD SHA at construction time (for incremental invalidation).
    pub last_commit: Option<String>,
    /// Staleness score in [0, 1] -- higher means less trustworthy.
    pub staleness: f32,
}

/// A row from the symbols table with enough data to build a chunk.
#[derive(Debug, Clone)]
pub struct SymbolRow {
    pub rowid: i64,
    pub file: String,
    pub name: String,
    pub kind: String,
    pub start_line: i64,
    pub end_line: i64,
    pub parent: Option<String>,
}

/// Build the chunk text for a symbol given its context.
///
/// The context window format:
/// ```text
/// [kind] name (parent if any)
/// path/to/file.rs:start_line
/// <doc comment prose>
///
/// Callers: foo, bar, baz
/// Callees: qux, quux
/// Co-changes with: some_file.rs, other_file.rs
/// ```
pub fn build_symbol_chunk(
    symbol: &SymbolRow,
    doc_comment: Option<&str>,
    callers: &[String],
    callees: &[String],
    co_change_files: &[String],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Signature line
    let sig = if let Some(parent) = &symbol.parent {
        format!(
            "[{}] {}.{} -- {}:{}",
            symbol.kind, parent, symbol.name, symbol.file, symbol.start_line
        )
    } else {
        format!(
            "[{}] {} -- {}:{}",
            symbol.kind, symbol.name, symbol.file, symbol.start_line
        )
    };
    parts.push(sig);

    // Doc comment prose (comment markers already stripped by caller)
    if let Some(doc) = doc_comment {
        let prose = collapse_line_wraps(doc.trim());
        if !prose.is_empty() {
            parts.push(prose);
        }
    }

    // Callers
    if !callers.is_empty() {
        let top: Vec<&String> = callers.iter().take(5).collect();
        parts.push(format!(
            "Callers: {}",
            top.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // Callees
    if !callees.is_empty() {
        let top: Vec<&String> = callees.iter().take(5).collect();
        parts.push(format!(
            "Callees: {}",
            top.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // Co-change neighbors
    if !co_change_files.is_empty() {
        let top: Vec<&String> = co_change_files.iter().take(5).collect();
        parts.push(format!(
            "Co-changes with: {}",
            top.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    parts.join("\n")
}

/// Build a chunk for a single markdown heading section.
///
/// Format:
/// ```text
/// [doc] path/to/README.md
/// Parent Heading > Section Heading
///
/// Section body text here.
/// ```
pub fn build_markdown_chunk(path: &str, heading_breadcrumb: &str, body: &str) -> String {
    let mut out = format!("[doc] {path}");
    if !heading_breadcrumb.is_empty() {
        out.push('\n');
        out.push_str(heading_breadcrumb);
    }
    let trimmed_body = body.trim();
    if !trimmed_body.is_empty() {
        out.push_str("\n\n");
        out.push_str(trimmed_body);
    }
    out
}

/// Parse a markdown file into (heading_breadcrumb, body) section pairs.
///
/// Each ATX heading (`#`, `##`, ...) starts a new section. The breadcrumb is
/// built by joining the heading hierarchy with ` > `. The root section (text
/// before the first heading) uses an empty breadcrumb.
pub fn split_markdown_sections(content: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    // Stack holds (level, title) for active headings.
    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    let mut current_body = String::new();
    let mut current_breadcrumb = String::new();

    for line in content.lines() {
        if line.starts_with('#') {
            // Count the leading '#' characters
            let level = line.chars().take_while(|&c| c == '#').count();
            let title = line.trim_start_matches('#').trim().to_string();

            // Flush current section
            if !current_body.trim().is_empty() || !current_breadcrumb.is_empty() {
                sections.push((current_breadcrumb.clone(), current_body.clone()));
            }

            // Pop headings of equal or deeper level from stack
            heading_stack.retain(|(l, _)| *l < level);
            heading_stack.push((level, title));

            // Build new breadcrumb from stack
            current_breadcrumb = heading_stack
                .iter()
                .map(|(_, t)| t.as_str())
                .collect::<Vec<_>>()
                .join(" > ");
            current_body = String::new();
        } else if !current_body.is_empty() || !line.trim().is_empty() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    // Flush final section
    if !current_body.trim().is_empty() || !current_breadcrumb.is_empty() {
        sections.push((current_breadcrumb, current_body));
    }

    sections
}

/// Build a chunk for a git commit message.
///
/// Format:
/// ```text
/// [commit] <hash>
/// Date: <ISO date>
/// <subject line>
///
/// <body>
/// ```
pub fn build_commit_chunk(hash: &str, date_str: &str, subject: &str, body: &str) -> String {
    let mut out = format!("[commit] {hash}\nDate: {date_str}\n{subject}");
    let trimmed = body.trim();
    if !trimmed.is_empty() {
        out.push_str("\n\n");
        out.push_str(trimmed);
    }
    out
}

/// Collapse soft line-wraps in prose. Single newlines within a paragraph
/// become spaces; double newlines (paragraph breaks) are preserved.
fn collapse_line_wraps(text: &str) -> String {
    let mut result = String::new();
    let mut last_empty = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !last_empty {
                result.push('\n');
            }
            last_empty = true;
        } else {
            if !result.is_empty() && !last_empty {
                result.push(' ');
            } else if !result.is_empty() && last_empty {
                result.push('\n');
            }
            result.push_str(trimmed);
            last_empty = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_line_wraps() {
        let text = "This is a long\nsentence that wraps.\n\nNew paragraph here.";
        let collapsed = collapse_line_wraps(text);
        assert!(collapsed.contains("This is a long sentence that wraps."));
        assert!(collapsed.contains("New paragraph here."));
    }

    #[test]
    fn test_build_symbol_chunk() {
        let sym = SymbolRow {
            rowid: 1,
            file: "src/lib.rs".to_string(),
            name: "open".to_string(),
            kind: "function".to_string(),
            start_line: 42,
            end_line: 60,
            parent: None,
        };
        let chunk = build_symbol_chunk(
            &sym,
            Some("Opens the database connection."),
            &["main".to_string()],
            &["connect".to_string()],
            &[],
        );
        assert!(chunk.contains("[function] open"));
        assert!(chunk.contains("Opens the database connection."));
        assert!(chunk.contains("Callers: main"));
        assert!(chunk.contains("Callees: connect"));
    }

    #[test]
    fn test_split_markdown_sections_basic() {
        let md = "# Title\n\nIntro text.\n\n## Section A\n\nBody A.\n\n## Section B\n\nBody B.\n";
        let sections = split_markdown_sections(md);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].0, "Title");
        assert!(sections[0].1.contains("Intro text."));
        assert_eq!(sections[1].0, "Title > Section A");
        assert!(sections[1].1.contains("Body A."));
        assert_eq!(sections[2].0, "Title > Section B");
        assert!(sections[2].1.contains("Body B."));
    }

    #[test]
    fn test_split_markdown_sections_empty() {
        let sections = split_markdown_sections("");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_build_markdown_chunk() {
        let chunk = build_markdown_chunk("docs/README.md", "Title > Section A", "Body text.");
        assert!(chunk.contains("[doc] docs/README.md"));
        assert!(chunk.contains("Title > Section A"));
        assert!(chunk.contains("Body text."));
    }

    #[test]
    fn test_build_commit_chunk() {
        let chunk = build_commit_chunk(
            "abc1234",
            "2026-01-15",
            "feat: add semantic search",
            "Longer description here.",
        );
        assert!(chunk.contains("[commit] abc1234"));
        assert!(chunk.contains("Date: 2026-01-15"));
        assert!(chunk.contains("feat: add semantic search"));
        assert!(chunk.contains("Longer description here."));
    }
}
