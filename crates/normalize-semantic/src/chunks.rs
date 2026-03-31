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
//! The quality of the embedding is directly upstream of the quality of the
//! index. Better extraction → better context windows → better embeddings.

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
    /// Staleness score in [0, 1] — higher means less trustworthy.
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
            "[{}] {}.{} — {}:{}",
            symbol.kind, parent, symbol.name, symbol.file, symbol.start_line
        )
    } else {
        format!(
            "[{}] {} — {}:{}",
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

/// Strip doc-comment markers from raw comment text.
///
/// Handles:
/// - `///` (Rust, C++, JS)
/// - `//!` (Rust inner doc)
/// - `--` (SQL, Lua)
/// - `#` (Python, Ruby, Shell)
/// - `/** ... */` and `/* ... */` block styles
/// - `"""..."""` (Python docstring body — already stripped of delimiters)
pub fn strip_doc_markers(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let content = if let Some(rest) = trimmed.strip_prefix("///") {
            rest.trim_start().to_string()
        } else if let Some(rest) = trimmed.strip_prefix("//!") {
            rest.trim_start().to_string()
        } else if let Some(rest) = trimmed.strip_prefix("/**") {
            rest.trim_start_matches('*').trim().to_string()
        } else if let Some(rest) = trimmed.strip_prefix("*/") {
            rest.trim().to_string()
        } else if let Some(rest) = trimmed.strip_prefix("* ") {
            rest.to_string()
        } else if trimmed == "*" {
            String::new()
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            rest.to_string()
        } else if trimmed == "#" {
            String::new()
        } else if let Some(rest) = trimmed.strip_prefix("-- ") {
            rest.to_string()
        } else {
            trimmed.to_string()
        };
        lines.push(content);
    }
    lines.join("\n")
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
    fn test_strip_doc_markers_rust() {
        let raw = "/// Opens the database.\n/// Returns an error if the file is corrupt.";
        let stripped = strip_doc_markers(raw);
        assert!(stripped.contains("Opens the database."));
        assert!(!stripped.contains("///"));
    }

    #[test]
    fn test_strip_doc_markers_block() {
        let raw = "/**\n * Computes the hash.\n * @param data input bytes\n */";
        let stripped = strip_doc_markers(raw);
        assert!(stripped.contains("Computes the hash."));
        assert!(!stripped.contains("/**"));
    }

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
}
