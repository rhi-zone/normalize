//! Symbol-aware file editing for normalize.
//!
//! Provides utilities to locate symbols within source files and apply targeted
//! text replacements — used by `normalize edit` to rewrite functions, methods,
//! and other named constructs without touching the rest of the file.

use normalize_facts::{Extractor, Symbol};
use normalize_languages::parsers::{grammar_loader, parse_with_grammar};
use normalize_languages::{Language, support_for_path};
use std::path::Path;
use streaming_iterator::StreamingIterator;

pub use normalize_languages::ContainerBody;

/// Result of finding a symbol in a file
#[derive(Debug)]
#[allow(dead_code)] // Fields used by Debug trait and for edit operations
pub struct SymbolLocation {
    pub name: String,
    pub kind: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub indent: String,
}

/// Convert a 1-based line number to byte offset in content.
/// Clamps to content length for safety (last line may not have trailing newline).
///
/// Uses `char_indices` to locate actual newline bytes, ensuring the returned
/// offset always lands on a valid UTF-8 character boundary regardless of
/// multi-byte characters or CRLF line endings.
pub fn line_to_byte(content: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    let target = line - 1; // number of newlines to skip
    let mut newlines_seen = 0usize;
    let mut i = 0usize;
    while i < content.len() {
        // SAFETY: we advance i only to char boundaries via char_indices
        let b = content.as_bytes()[i];
        if b == b'\n' {
            newlines_seen += 1;
            if newlines_seen == target {
                return (i + 1).min(content.len());
            }
        }
        // Advance by char width to stay on boundaries
        let ch_len = content[i..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        i += ch_len;
    }
    content.len()
}

/// Editor for structural code modifications
pub struct Editor {}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {}
    }

    /// Find a symbol by name in a file (uses skeleton extractor)
    pub fn find_symbol(
        &self,
        path: &Path,
        content: &str,
        name: &str,
        case_insensitive: bool,
    ) -> Option<SymbolLocation> {
        let extractor = Extractor::new();
        let result = extractor.extract(path, content);

        fn search_symbols(
            symbols: &[Symbol],
            name: &str,
            content: &str,
            case_insensitive: bool,
        ) -> Option<SymbolLocation> {
            for sym in symbols {
                let matches = if case_insensitive {
                    sym.name.eq_ignore_ascii_case(name)
                } else {
                    sym.name == name
                };
                if matches {
                    let start_byte = line_to_byte(content, sym.start_line);
                    let end_byte = line_to_byte(content, sym.end_line + 1);

                    return Some(SymbolLocation {
                        name: sym.name.clone(),
                        kind: sym.kind.as_str().to_string(),
                        start_byte,
                        end_byte,
                        start_line: sym.start_line,
                        end_line: sym.end_line,
                        indent: String::new(),
                    });
                }
                // Search children
                if let Some(loc) = search_symbols(&sym.children, name, content, case_insensitive) {
                    return Some(loc);
                }
            }
            None
        }

        search_symbols(&result.symbols, name, content, case_insensitive)
    }

    /// Delete a symbol from the content
    pub fn delete_symbol(&self, content: &str, loc: &SymbolLocation) -> String {
        let mut result = String::new();

        // Find the start of the line containing the symbol, then walk back to include
        // any preceding doc comments and attributes/decorators.
        let raw_line_start = content[..loc.start_byte]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_start = extend_to_decorations(content, raw_line_start);

        // Find the end of the line containing the symbol end (include trailing newline)
        let mut end_byte = loc.end_byte;
        if end_byte < content.len() && content.as_bytes()[end_byte] == b'\n' {
            end_byte += 1;
        }

        // Smart whitespace: consume trailing blank lines to avoid double-blanks
        // But only if there's already a blank line before the symbol
        let has_blank_before =
            line_start >= 2 && &content[line_start.saturating_sub(2)..line_start] == "\n\n";

        if has_blank_before {
            // Consume trailing blank lines (up to one full blank line)
            while end_byte < content.len() && content.as_bytes()[end_byte] == b'\n' {
                end_byte += 1;
                // Only consume one blank line worth
                if end_byte < content.len() && content.as_bytes()[end_byte] != b'\n' {
                    break;
                }
            }
        }

        result.push_str(&content[..line_start]);
        result.push_str(&content[end_byte..]);

        result
    }

    /// Replace a symbol with new content
    pub fn replace_symbol(&self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        result.push_str(&content[..loc.start_byte]);
        result.push_str(&indented);
        result.push_str(&content[loc.end_byte..]);

        result
    }

    /// Count blank lines before a position
    fn count_blank_lines_before(&self, content: &str, pos: usize) -> usize {
        let mut count = 0usize;
        let mut i = pos;
        while i > 0 {
            i -= 1;
            if content.as_bytes()[i] == b'\n' {
                count += 1;
            } else if !content.as_bytes()[i].is_ascii_whitespace() {
                break;
            }
        }
        count.saturating_sub(1) // Don't count the newline ending the previous line
    }

    /// Count blank lines after a position (after any trailing newline)
    fn count_blank_lines_after(&self, content: &str, pos: usize) -> usize {
        let mut count = 0;
        let mut i = pos;
        // Skip past the first newline (end of current symbol)
        if i < content.len() && content.as_bytes()[i] == b'\n' {
            i += 1;
        }
        while i < content.len() {
            if content.as_bytes()[i] == b'\n' {
                count += 1;
                i += 1;
            } else if content.as_bytes()[i].is_ascii_whitespace() {
                i += 1;
            } else {
                break;
            }
        }
        count
    }

    /// Insert content before a symbol
    pub fn insert_before(&self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Find the start of the line containing the symbol
        let line_start = content[..loc.start_byte]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        // Detect spacing convention: how many blank lines before this symbol?
        let blank_lines = self.count_blank_lines_before(content, line_start);
        // +1 for the newline ending the content, +N for N blank lines
        let spacing = "\n".repeat(blank_lines.max(1) + 1);

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        result.push_str(&content[..line_start]);
        result.push_str(&indented);
        result.push_str(&spacing);
        result.push_str(&content[line_start..]);

        result
    }

    /// Insert content after a symbol
    pub fn insert_after(&self, content: &str, loc: &SymbolLocation, new_content: &str) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &loc.indent);

        // Find the end of the symbol (include trailing newline)
        let end_pos = if loc.end_byte < content.len() && content.as_bytes()[loc.end_byte] == b'\n' {
            loc.end_byte + 1
        } else {
            loc.end_byte
        };

        // Detect spacing convention: how many blank lines after this symbol?
        let blank_lines = self.count_blank_lines_after(content, loc.end_byte);
        // end_pos already includes trailing newline, so just add N newlines for N blank lines
        let spacing = "\n".repeat(blank_lines.max(1));

        // Find where the next non-blank content starts
        let mut next_content_pos = end_pos;
        while next_content_pos < content.len() && content.as_bytes()[next_content_pos] == b'\n' {
            next_content_pos += 1;
        }

        result.push_str(&content[..end_pos]);
        result.push_str(&spacing);
        result.push_str(&indented);

        if next_content_pos < content.len() {
            // +1 for the newline ending the inserted content
            result.push_str(&"\n".repeat(blank_lines.max(1) + 1));
            result.push_str(&content[next_content_pos..]);
        } else {
            result.push('\n');
        }

        result
    }

    /// Insert content at the beginning of a file
    pub fn prepend_to_file(&self, content: &str, new_content: &str) -> String {
        let mut result = String::new();
        result.push_str(new_content);
        if !new_content.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(content);
        result
    }

    /// Insert content at the end of a file
    pub fn append_to_file(&self, content: &str, new_content: &str) -> String {
        let mut result = String::new();
        result.push_str(content);
        if !content.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(new_content);
        if !new_content.ends_with('\n') {
            result.push('\n');
        }
        result
    }

    /// Find the body of a container symbol (class, impl block, markdown section) for prepend/append
    pub fn find_container_body(
        &self,
        path: &Path,
        content: &str,
        name: &str,
    ) -> Option<ContainerBody> {
        let support = support_for_path(path)?;
        let grammar = support.grammar_name();
        let tree = parse_with_grammar(grammar, content)?;

        // Use the tags query to locate container nodes by name.
        let loader = grammar_loader();
        let tags_scm = loader.get_tags(grammar)?;
        let ts_lang = loader.get(grammar).ok()?;
        let tags_query = tree_sitter::Query::new(&ts_lang, &tags_scm).ok()?;
        find_container_body_via_tags(&tree, &tags_query, content, name, support)
    }

    /// Prepend content inside a container (class/impl body)
    pub fn prepend_to_container(
        &self,
        content: &str,
        body: &ContainerBody,
        new_content: &str,
    ) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &body.inner_indent);

        result.push_str(&content[..body.content_start]);

        // Add the new content
        result.push_str(&indented);
        result.push('\n');

        // Add spacing if there's existing content
        if !body.is_empty {
            result.push('\n');
        }

        result.push_str(&content[body.content_start..]);

        result
    }

    /// Append content inside a container (class/impl body)
    pub fn append_to_container(
        &self,
        content: &str,
        body: &ContainerBody,
        new_content: &str,
    ) -> String {
        let mut result = String::new();

        // Apply indentation to new content
        let indented = self.apply_indent(new_content, &body.inner_indent);

        // Trim trailing whitespace/newlines from existing content
        let mut end_pos = body.content_end;
        while end_pos > 0
            && content
                .as_bytes()
                .get(end_pos - 1)
                .map(|&b| b == b'\n' || b == b' ')
                == Some(true)
        {
            end_pos -= 1;
        }

        result.push_str(&content[..end_pos]);

        // Add blank line before new content (Python/Rust convention for methods)
        if !body.is_empty {
            result.push_str("\n\n");
        } else {
            result.push('\n');
        }

        // Add the new content
        result.push_str(&indented);
        result.push('\n');

        result.push_str(&content[body.content_end..]);

        result
    }

    /// Rename all word-boundary occurrences of `old_name` on a specific line (1-based).
    ///
    /// Replaces every whole-word occurrence of `old_name` on that line with `new_name`.
    /// Returns `None` if the line number is out of range or if `old_name` does not
    /// appear as a whole word anywhere on that line.
    pub fn rename_identifier_in_line(
        &self,
        content: &str,
        line_no: usize,
        old_name: &str,
        new_name: &str,
    ) -> Option<String> {
        let (line_start, line_end) = line_byte_range(content, line_no)?;
        let line = &content[line_start..line_end];
        let new_line = replace_all_words(line, old_name, new_name);
        if new_line == line {
            return None;
        }
        let mut result = String::with_capacity(content.len() + new_name.len() * 4);
        result.push_str(&content[..line_start]);
        result.push_str(&new_line);
        result.push_str(&content[line_end..]);
        Some(result)
    }

    /// Apply indentation to content
    pub fn apply_indent(&self, content: &str, indent: &str) -> String {
        content
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    format!("{}{}", indent, line)
                } else if line.is_empty() {
                    line.to_string()
                } else {
                    format!("{}{}", indent, line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Returns the byte range [start, end) of the Nth (1-based) line in `content`,
/// not including the trailing newline. Returns `None` if `line_no` is out of range.
fn line_byte_range(content: &str, line_no: usize) -> Option<(usize, usize)> {
    if line_no == 0 {
        return None;
    }
    let mut start = 0usize;
    let mut current_line = 1usize;
    for (i, c) in content.char_indices() {
        if current_line == line_no {
            // start is set; find end
            let end = content[i..]
                .find('\n')
                .map(|n| i + n)
                .unwrap_or(content.len());
            return Some((start, end));
        }
        if c == '\n' {
            current_line += 1;
            start = i + 1;
        }
    }
    // Handle single-line file with no newline
    if current_line == line_no {
        Some((start, content.len()))
    } else {
        None
    }
}

/// Replace all whole-word occurrences of `old` in `text` with `new_word`.
/// Returns the original string unchanged if no occurrences are found.
fn replace_all_words(text: &str, old: &str, new_word: &str) -> String {
    if old.is_empty() {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len());
    let mut offset = 0;
    loop {
        match text[offset..].find(old) {
            None => {
                result.push_str(&text[offset..]);
                break;
            }
            Some(pos) => {
                let abs = offset + pos;
                let before_ok = abs == 0 || {
                    let b = bytes[abs - 1];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                let after = abs + old.len();
                let after_ok = after >= bytes.len() || {
                    let b = bytes[after];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                if before_ok && after_ok {
                    result.push_str(&text[offset..abs]);
                    result.push_str(new_word);
                    offset = after;
                } else {
                    // Not a word boundary — copy one char and keep searching
                    let next = text[abs..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    result.push_str(&text[offset..abs + next]);
                    offset = abs + next;
                }
            }
        }
    }
    result
}

/// Find a container body using a tags query.
///
/// Used when the language has a `*.tags.scm`.
/// Runs the tags query to find `@definition.class`, `@definition.module`, or
/// `@definition.interface` nodes whose name matches `name`, then delegates to
/// the Language trait's `container_body` / `analyze_container_body` methods.
fn find_container_body_via_tags(
    tree: &tree_sitter::Tree,
    tags_query: &tree_sitter::Query,
    content: &str,
    name: &str,
    support: &dyn Language,
) -> Option<ContainerBody> {
    let capture_names = tags_query.capture_names();

    let root = tree.root_node();
    let mut qcursor = tree_sitter::QueryCursor::new();
    let mut matches = qcursor.matches(tags_query, root, content.as_bytes());

    while let Some(m) = matches.next() {
        for capture in m.captures {
            let cn = capture_names[capture.index as usize];
            if !matches!(
                cn,
                "definition.class" | "definition.module" | "definition.interface"
            ) {
                continue;
            }
            let node = capture.node;
            let container_name = support.node_name(&node, content)?;
            if container_name != name {
                continue;
            }
            let body_node = support.container_body(&node)?;
            let start_byte = node.start_byte();
            let line_start = content[..start_byte]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let container_indent: String = content[line_start..start_byte]
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            let inner_indent = format!("{}    ", container_indent);
            if let Some(body) = support.analyze_container_body(&body_node, content, &inner_indent) {
                return Some(body);
            }
        }
    }

    None
}

/// Walk backwards from `line_start` to include any preceding doc comments, attributes,
/// or decorators that belong to the symbol at that position.
///
/// Recognises the following line prefixes (after trimming whitespace):
/// - `///`, `//!`, `/**`, `/*` — Rust/C/Java/JS/TS doc comments
/// - `//` — any line comment
/// - `#[` — Rust attributes
/// - `#` — Python/Ruby/Bash comments and decorators
/// - `@` — Python/Java/JS/TS/Zig decorators / annotations
/// - `--` — Lua/SQL/Haskell line comments
/// - blank lines immediately adjacent to the decoration block (up to one blank line)
///
/// Stops when a line is none of the above (ordinary code, another symbol definition).
/// This is purely textual — no tree-sitter, no language detection.
pub fn extend_to_decorations(content: &str, line_start: usize) -> usize {
    if line_start == 0 {
        return 0;
    }

    let mut result = line_start;
    // Allow one blank line between the decoration block and the item itself.
    let mut blank_allowance: u8 = 1;

    // `pos` starts just before the '\n' that ends the line above `line_start`.
    let mut pos = line_start.saturating_sub(1);
    if pos == 0 {
        return result;
    }

    loop {
        let prev_line_start = content[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = content[prev_line_start..pos].trim();

        if line.is_empty() {
            if blank_allowance > 0 {
                blank_allowance -= 1;
                result = prev_line_start;
                if prev_line_start == 0 {
                    break;
                }
                pos = prev_line_start.saturating_sub(1);
                continue;
            } else {
                break;
            }
        }

        let is_decoration = line.starts_with("///")
            || line.starts_with("//!")
            || line.starts_with("/**")
            || line.starts_with("/*")
            || line.starts_with("//")
            || line.starts_with("#[")
            || line.starts_with('#')
            || line.starts_with('@')
            || line.starts_with("--");

        if is_decoration {
            // Reset blank allowance — we found another decoration line above.
            blank_allowance = 1;
            result = prev_line_start;
            if prev_line_start == 0 {
                break;
            }
            pos = prev_line_start.saturating_sub(1);
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_find_python_function() {
        let editor = Editor::new();
        let content = r#"
def foo():
    pass

def bar():
    return 42
"#;
        let loc = editor.find_symbol(&PathBuf::from("test.py"), content, "bar", false);
        assert!(loc.is_some());
        let loc = loc.unwrap();
        assert_eq!(loc.name, "bar");
        assert_eq!(loc.kind, "function");
    }

    #[test]
    fn test_delete_symbol() {
        let editor = Editor::new();
        let content = "def foo():\n    pass\n\ndef bar():\n    return 42\n";
        let loc = editor
            .find_symbol(&PathBuf::from("test.py"), content, "bar", false)
            .unwrap();
        let result = editor.delete_symbol(content, &loc);
        assert!(!result.contains("bar"));
        assert!(result.contains("foo"));
    }

    #[test]
    fn test_insert_before() {
        let editor = Editor::new();
        let content = "def foo():\n    pass\n\ndef bar():\n    return 42\n";
        let loc = editor
            .find_symbol(&PathBuf::from("test.py"), content, "bar", false)
            .unwrap();
        let result = editor.insert_before(content, &loc, "def baz():\n    pass");
        assert!(result.contains("baz"));
        assert!(result.find("baz").unwrap() < result.find("bar").unwrap());
    }

    #[test]
    fn test_prepend_to_python_class() {
        let editor = Editor::new();
        let content = r#"class Foo:
    """Docstring."""

    def first(self):
        pass
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.py"), content, "Foo")
            .unwrap();
        let result =
            editor.prepend_to_container(content, &body, "def new_method(self):\n    return 1");
        // New method should appear after docstring but before first
        assert!(result.contains("new_method"));
        let docstring_pos = result.find("Docstring").unwrap();
        let new_method_pos = result.find("new_method").unwrap();
        let first_pos = result.find("first").unwrap();
        assert!(docstring_pos < new_method_pos);
        assert!(new_method_pos < first_pos);
    }

    #[test]
    fn test_append_to_python_class() {
        let editor = Editor::new();
        let content = r#"class Foo:
    def first(self):
        pass

    def second(self):
        return 42
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.py"), content, "Foo")
            .unwrap();
        let result = editor.append_to_container(content, &body, "def last(self):\n    return 99");
        // New method should appear after second
        assert!(result.contains("last"));
        let second_pos = result.find("second").unwrap();
        let last_pos = result.find("last").unwrap();
        assert!(second_pos < last_pos);
    }

    #[test]
    fn test_prepend_to_rust_impl() {
        let editor = Editor::new();
        let content = r#"impl Foo {
    fn first(&self) -> i32 {
        1
    }
}
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.rs"), content, "Foo")
            .unwrap();
        let result =
            editor.prepend_to_container(content, &body, "fn new() -> Self {\n    Self {}\n}");
        assert!(result.contains("new"));
        let new_pos = result.find("new").unwrap();
        let first_pos = result.find("first").unwrap();
        assert!(new_pos < first_pos);
    }

    #[test]
    fn test_append_to_rust_impl() {
        let editor = Editor::new();
        let content = r#"impl Foo {
    fn first(&self) -> i32 {
        1
    }
}
"#;
        let body = editor
            .find_container_body(&PathBuf::from("test.rs"), content, "Foo")
            .unwrap();
        let result =
            editor.append_to_container(content, &body, "fn last(&self) -> i32 {\n    99\n}");
        assert!(result.contains("last"));
        let first_pos = result.find("first").unwrap();
        let last_pos = result.find("last").unwrap();
        assert!(first_pos < last_pos);
        // Should still have closing brace
        assert!(result.contains("}"));
    }
}
