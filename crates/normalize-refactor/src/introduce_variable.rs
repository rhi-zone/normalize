//! Introduce-variable recipe: extract an expression at a given range into a named variable binding.
//!
//! Algorithm:
//! 1. Parse the file with tree-sitter.
//! 2. Find the innermost complete expression node covering the given byte range.
//! 3. Walk up to the parent statement of that expression.
//! 4. Insert `let <name> = <expr>;` (or the language-appropriate binding) before the statement.
//! 5. Replace the original expression text with `<name>`.
//!
//! Language-specific keyword mapping (everything else uses `let`):
//! - Python: `<name> = <expr>` (no keyword)
//! - JavaScript / TypeScript: `const <name> = <expr>;`
//! - Rust, Go, Swift, Kotlin, Scala, Dart, etc.: `let <name> = <expr>;`

use std::path::Path;

use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::support_for_path;

use crate::{PlannedEdit, RefactoringPlan};

/// A byte range selected by the user.
#[derive(Debug, Clone, Copy)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

/// Outcome of a successful introduce-variable plan.
pub struct IntroduceVariableOutcome {
    pub plan: RefactoringPlan,
    /// The variable name that was introduced.
    pub name: String,
    /// 1-based line number where the `let` binding was inserted.
    pub inserted_line: usize,
    /// Byte range that was replaced with the variable name.
    pub replaced_start: usize,
    pub replaced_end: usize,
}

/// Build an introduce-variable plan without touching the filesystem.
///
/// `file` is the absolute path to the file.
/// `content` is the current file content.
/// `range` is the byte range of the expression to extract.
/// `name` is the variable name to introduce.
pub fn plan_introduce_variable(
    file: &Path,
    content: &str,
    range: ByteRange,
    name: &str,
) -> Result<IntroduceVariableOutcome, String> {
    // Validate range bounds.
    if range.start > range.end || range.end > content.len() {
        return Err(format!(
            "Invalid range {}..{} for file of length {}",
            range.start,
            range.end,
            content.len()
        ));
    }

    // Determine grammar from path.
    let support = support_for_path(file)
        .ok_or_else(|| format!("No language support for {}", file.display()))?;
    let grammar = support.grammar_name();

    let tree = parse_with_grammar(grammar, content).ok_or_else(|| {
        format!(
            "Grammar '{}' not available — install grammars with `normalize grammars install`",
            grammar
        )
    })?;

    let root = tree.root_node();

    // Find the innermost node that covers the selection range.
    let expr_node = root
        .descendant_for_byte_range(range.start, range.end)
        .ok_or_else(|| {
            format!(
                "No AST node found at byte range {}..{}",
                range.start, range.end
            )
        })?;

    // Validate that the selected range corresponds to a reasonably complete expression.
    // The node should start at or before the selection start and end at or after end.
    let node_start = expr_node.start_byte();
    let node_end = expr_node.end_byte();

    // If the node's range doesn't match the selection closely (allowing for whitespace),
    // search upward for a better match (a node whose range closely contains the selection).
    let expr_node = find_best_expression_node(expr_node, range);

    let actual_start = expr_node.start_byte();
    let actual_end = expr_node.end_byte();

    // The selected text must be a meaningful expression (not purely structural tokens).
    let selected_text = content[actual_start..actual_end].trim();
    if selected_text.is_empty() {
        return Err("Selected range is empty or whitespace only".to_string());
    }

    // Verify the node kind looks like an expression (not a statement wrapper, keyword, etc.).
    let kind = expr_node.kind();
    if is_statement_kind(kind) {
        return Err(format!(
            "Selected node '{}' is a statement, not an expression. Select the expression inside it.",
            kind
        ));
    }

    // Unused — suppressed by the assignment above which overwrites it.
    let _ = (node_start, node_end);

    // Walk up to find the parent statement that contains this expression.
    let stmt_node = find_parent_statement(&expr_node)
        .ok_or_else(|| "Could not find a parent statement for the expression".to_string())?;

    // Determine indentation of the statement.
    let stmt_start = stmt_node.start_byte();
    let indent = leading_whitespace(content, stmt_start);

    // Generate the binding declaration.
    let expr_text = content[actual_start..actual_end].to_string();
    let binding = make_binding(grammar, name, &expr_text, &indent);

    // Build new file content:
    // 1. Insert the binding before the statement line.
    // 2. Replace the expression with the variable name.
    //
    // We must do these two edits carefully because inserting text shifts byte offsets.
    // Strategy: apply the replacement first (it's inside the statement), then insert
    // before the statement. Because the insertion is before the replacement site, we
    // need to do insertion first and adjust the replacement offset.

    // Compute the byte position of the start of the statement's line.
    let insert_pos = line_start(content, stmt_start);

    // After inserting `binding` at `insert_pos`, the expression bytes shift by `binding.len()`.
    let new_expr_start = actual_start + binding.len();
    let new_expr_end = actual_end + binding.len();

    let mut new_content = content.to_string();
    // Insert binding before the statement line.
    new_content.insert_str(insert_pos, &binding);
    // Replace the expression with the variable name.
    new_content.replace_range(new_expr_start..new_expr_end, name);

    // Compute the 1-based line number of the inserted binding.
    let inserted_line = content[..insert_pos].chars().filter(|&c| c == '\n').count() + 1;

    let plan = RefactoringPlan {
        operation: "introduce_variable".to_string(),
        edits: vec![PlannedEdit {
            file: file.to_path_buf(),
            original: content.to_string(),
            new_content,
            description: format!("introduce variable '{}'", name),
        }],
        warnings: vec![],
    };

    Ok(IntroduceVariableOutcome {
        plan,
        name: name.to_string(),
        inserted_line,
        replaced_start: actual_start,
        replaced_end: actual_end,
    })
}

/// Walk up the tree to find the node whose range best matches the selection.
///
/// We prefer the most specific (innermost) node whose byte range exactly covers
/// the trimmed selection. If the direct match is inside a larger expression that
/// exactly matches, prefer the exact match.
fn find_best_expression_node<'a>(
    mut node: tree_sitter::Node<'a>,
    range: ByteRange,
) -> tree_sitter::Node<'a> {
    // If the node already exactly covers the range, keep it.
    if node.start_byte() == range.start && node.end_byte() == range.end {
        return node;
    }

    // Walk up while the parent is a better (closer) match for the range.
    loop {
        let Some(parent) = node.parent() else { break };
        // If the parent exactly covers the range, prefer it (it's the "expression" the
        // user intends rather than an inner token).
        if parent.start_byte() == range.start && parent.end_byte() == range.end {
            node = parent;
            continue;
        }
        // If the parent covers more than the range, stop — the current node is the
        // innermost covering node.
        if parent.start_byte() <= range.start && parent.end_byte() >= range.end {
            break;
        }
        break;
    }

    node
}

/// Returns true if the node kind is a statement wrapper, not an expression.
fn is_statement_kind(kind: &str) -> bool {
    matches!(
        kind,
        // Rust
        "let_declaration"
            | "expression_statement"
            // Python
            | "assignment"
            | "augmented_assignment"
            | "assert_statement"
            | "return_statement"
            | "pass_statement"
            | "break_statement"
            | "continue_statement"
            | "delete_statement"
            | "import_statement"
            | "import_from_statement"
            | "raise_statement"
            | "global_statement"
            | "nonlocal_statement"
            // JS/TS (not already covered above)
            | "lexical_declaration"
            | "variable_declaration"
            | "throw_statement"
            | "if_statement"
            | "while_statement"
            | "for_statement"
            | "for_in_statement"
            | "switch_statement"
            | "try_statement"
            // General
            | "block"
            | "source_file"
            | "program"
            | "module"
    )
}

/// Return the statement-level parent of an expression node.
///
/// Walks up the tree until we find a node that is at the statement level
/// (i.e., its parent is a block / function body / module).
fn find_parent_statement<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    let mut current = *node;
    loop {
        let Some(parent) = current.parent() else {
            // Reached root without finding a statement — return current (the
            // expression itself, which lives at the top level).
            return Some(current);
        };
        let parent_kind = parent.kind();
        if is_block_kind(parent_kind) {
            // current is a direct child of a block → it IS the statement.
            return Some(current);
        }
        current = parent;
    }
}

/// Returns true if the node kind is a block / body container.
fn is_block_kind(kind: &str) -> bool {
    matches!(
        kind,
        // Rust
        "block"
            // Python
            | "module"
            | "body"
            // JS / TS
            | "program"
            | "statement_block"
            // Generic
            | "source_file"
            | "class_body"
            | "enum_body"
    )
}

/// Return the byte position of the start of the line containing `pos`.
fn line_start(content: &str, pos: usize) -> usize {
    content[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// Extract the leading whitespace from the line containing `pos`.
fn leading_whitespace(content: &str, pos: usize) -> String {
    let ls = line_start(content, pos);
    let line = &content[ls..];
    let ws_end = line
        .find(|c: char| !c.is_whitespace())
        .unwrap_or(line.len());
    line[..ws_end].to_string()
}

/// Generate the variable binding declaration for the given grammar/language.
fn make_binding(grammar: &str, name: &str, expr: &str, indent: &str) -> String {
    match grammar {
        "python" => {
            // Python: `name = expr\n`
            format!("{}{} = {}\n", indent, name, expr)
        }
        "javascript" | "typescript" | "tsx" => {
            // JS/TS: `const name = expr;\n`
            format!("{}const {} = {};\n", indent, name, expr)
        }
        _ => {
            // Default (Rust, Go, Swift, Kotlin, etc.): `let name = expr;\n`
            format!("{}let {} = {};\n", indent, name, expr)
        }
    }
}

// ── Range parsing helpers ─────────────────────────────────────────────────────

/// Parse a `line:col-line:col` range string into a byte range.
///
/// Lines and columns are **1-based** (matching editor conventions).
/// Returns `Err` with a descriptive message on any parse failure.
pub fn parse_line_col_range(content: &str, range_str: &str) -> Result<ByteRange, String> {
    // Expected format: `<start_line>:<start_col>-<end_line>:<end_col>`
    let (start_part, end_part) = range_str.split_once('-').ok_or_else(|| {
        format!(
            "Invalid range '{}': expected format start_line:start_col-end_line:end_col",
            range_str
        )
    })?;

    let (sl, sc) = parse_line_col(start_part, range_str)?;
    let (el, ec) = parse_line_col(end_part, range_str)?;

    let start_byte = line_col_to_byte(content, sl, sc).ok_or_else(|| {
        format!(
            "Start {}:{} is out of bounds for file of {} chars",
            sl,
            sc,
            content.len()
        )
    })?;
    let end_byte = line_col_to_byte(content, el, ec).ok_or_else(|| {
        format!(
            "End {}:{} is out of bounds for file of {} chars",
            el,
            ec,
            content.len()
        )
    })?;

    if start_byte > end_byte {
        return Err(format!(
            "Start byte {} > end byte {} — range is backwards",
            start_byte, end_byte
        ));
    }

    Ok(ByteRange {
        start: start_byte,
        end: end_byte,
    })
}

fn parse_line_col(s: &str, full: &str) -> Result<(usize, usize), String> {
    let (line_s, col_s) = s.split_once(':').ok_or_else(|| {
        format!(
            "Invalid position '{}' in range '{}': expected line:col",
            s, full
        )
    })?;
    let line: usize = line_s
        .parse()
        .map_err(|_| format!("Invalid line number '{}' in range '{}'", line_s, full))?;
    let col: usize = col_s
        .parse()
        .map_err(|_| format!("Invalid column number '{}' in range '{}'", col_s, full))?;
    if line == 0 || col == 0 {
        return Err(format!(
            "Line and column numbers are 1-based; got {}:{} in range '{}'",
            line, col, full
        ));
    }
    Ok((line, col))
}

/// Convert a 1-based line:col pair to a byte offset in `content`.
fn line_col_to_byte(content: &str, line: usize, col: usize) -> Option<usize> {
    let mut current_line = 1usize;
    let mut current_col = 1usize;
    for (byte_pos, ch) in content.char_indices() {
        if current_line == line && current_col == col {
            return Some(byte_pos);
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 1;
        } else {
            current_col += 1;
        }
    }
    // Allow pointing at end of content (e.g. end of last line without newline).
    if current_line == line && current_col == col {
        return Some(content.len());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rust_file() -> PathBuf {
        PathBuf::from("test.rs")
    }

    fn py_file() -> PathBuf {
        PathBuf::from("test.py")
    }

    fn ts_file() -> PathBuf {
        PathBuf::from("test.ts")
    }

    fn js_file() -> PathBuf {
        PathBuf::from("test.js")
    }

    // Helper: get byte range for a substring (first occurrence).
    fn byte_range_of(content: &str, needle: &str) -> ByteRange {
        let start = content
            .find(needle)
            .unwrap_or_else(|| panic!("needle {:?} not found in content: {:?}", needle, content));
        ByteRange {
            start,
            end: start + needle.len(),
        }
    }

    #[test]
    fn test_rust_introduce_variable() {
        let content = "fn main() {\n    let result = some_function(x + y * 2);\n}\n";
        let range = byte_range_of(content, "x + y * 2");
        let outcome = plan_introduce_variable(&rust_file(), content, range, "sum").unwrap();
        assert_eq!(outcome.name, "sum");
        let new_content = &outcome.plan.edits[0].new_content;
        assert!(
            new_content.contains("let sum = x + y * 2;"),
            "expected let binding, got:\n{}",
            new_content
        );
        assert!(
            new_content.contains("some_function(sum)"),
            "expected expression replaced, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_python_introduce_variable() {
        let content = "def main():\n    result = some_function(x + y * 2)\n    print(result)\n";
        let range = byte_range_of(content, "x + y * 2");
        let outcome = plan_introduce_variable(&py_file(), content, range, "total").unwrap();
        let new_content = &outcome.plan.edits[0].new_content;
        // Python uses `name = expr` (no let)
        assert!(
            new_content.contains("total = x + y * 2"),
            "expected python binding, got:\n{}",
            new_content
        );
        assert!(
            new_content.contains("some_function(total)"),
            "expected expression replaced, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_typescript_introduce_variable() {
        let content = "function main() {\n    const result = someFunction(x + y * 2);\n    console.log(result);\n}\n";
        let range = byte_range_of(content, "x + y * 2");
        let outcome = plan_introduce_variable(&ts_file(), content, range, "sum").unwrap();
        let new_content = &outcome.plan.edits[0].new_content;
        assert!(
            new_content.contains("const sum = x + y * 2;"),
            "expected const binding, got:\n{}",
            new_content
        );
        assert!(
            new_content.contains("someFunction(sum)"),
            "expected expression replaced, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_javascript_introduce_variable() {
        let content = "function main() {\n    const result = someFunction(x + y * 2);\n    console.log(result);\n}\n";
        let range = byte_range_of(content, "x + y * 2");
        let outcome = plan_introduce_variable(&js_file(), content, range, "sum").unwrap();
        let new_content = &outcome.plan.edits[0].new_content;
        assert!(
            new_content.contains("const sum = x + y * 2;"),
            "expected const binding, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_indentation_preserved() {
        let content = "fn main() {\n    if true {\n        let x = foo(a + b);\n    }\n}\n";
        let range = byte_range_of(content, "a + b");
        let outcome = plan_introduce_variable(&rust_file(), content, range, "sum").unwrap();
        let new_content = &outcome.plan.edits[0].new_content;
        // Should preserve the 8-space indent of the statement.
        assert!(
            new_content.contains("        let sum = a + b;"),
            "expected indented binding, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_parse_line_col_range() {
        let content = "fn main() {\n    let x = 1;\n}\n";
        // "let" starts at line 2, col 5
        let range = parse_line_col_range(content, "2:5-2:8").unwrap();
        assert_eq!(&content[range.start..range.end], "let");
    }

    #[test]
    fn test_error_on_statement_selection() {
        let content = "fn main() {\n    let x = 1 + 2;\n}\n";
        // Select the entire let_declaration
        let range = byte_range_of(content, "let x = 1 + 2;");
        let result = plan_introduce_variable(&rust_file(), content, range, "y");
        assert!(result.is_err(), "should error on statement selection");
    }
}
