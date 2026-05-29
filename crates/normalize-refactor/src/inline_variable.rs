//! Inline-variable recipe: replace all uses of a variable with its initializer and remove the binding.
//!
//! Algorithm:
//! 1. Parse the file with tree-sitter.
//! 2. Locate the variable declaration node at the given position (line:col of the variable name).
//! 3. Extract: variable name, initializer expression text, declaration node byte range,
//!    and the scope node (the function/block that contains the declaration).
//! 4. Within that scope: walk all identifier nodes, collect those whose text matches the
//!    variable name (conservative: skip if ambiguous).
//! 5. Check for reassignments — error out if any exist.
//! 6. Replace each reference with the initializer expression text (wrapping in parens if needed).
//! 7. Remove the declaration statement line.
//!
//! Declaration nodes are classified by the `@refactor.var_decl` capture and
//! reassignment targets by `@refactor.reassign`; scopes/blocks by
//! `@refactor.scope`/`@refactor.block`. The structural navigation that finds the
//! binding identifier and the initializer within a declaration uses tree-sitter
//! field names (extraction), trying the binding strategies the supported grammars
//! use (Rust `let`, JS/TS `variable_declarator`, Python `assignment`).

use std::path::Path;

use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::support_for_path;

use crate::refactor_query::RefactorCaptures;
use crate::{PlannedEdit, RefactoringPlan};

/// Outcome of a successful inline-variable plan.
pub struct InlineVariableOutcome {
    pub plan: RefactoringPlan,
    /// The variable name that was inlined.
    pub name: String,
    /// Number of use-sites replaced (not counting the declaration removal).
    pub references_replaced: usize,
}

/// Build an inline-variable plan without touching the filesystem.
///
/// `file` is the absolute path to the file (used for language detection).
/// `content` is the current file content.
/// `line` and `col` are 1-based (pointing at the variable name in its declaration).
pub fn plan_inline_variable(
    file: &Path,
    content: &str,
    line: usize,
    col: usize,
) -> Result<InlineVariableOutcome, String> {
    if line == 0 || col == 0 {
        return Err("Line and column numbers are 1-based".to_string());
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

    let root_node = tree.root_node();

    let caps = RefactorCaptures::load(grammar, root_node, content).ok_or_else(|| {
        format!(
            "inline-variable does not support language {} (no refactor query)",
            support.name()
        )
    })?;

    // Convert line:col to byte offset.
    let target_byte = line_col_to_byte(content, line, col).ok_or_else(|| {
        format!(
            "Position {}:{} is out of bounds for file of {} bytes",
            line,
            col,
            content.len()
        )
    })?;

    // Find the identifier node at the given position.
    let ident_node = root_node
        .descendant_for_byte_range(target_byte, target_byte + 1)
        .ok_or_else(|| format!("No AST node found at {}:{}", line, col))?;

    if ident_node.kind() != "identifier" {
        return Err(format!(
            "Position {}:{} points to a '{}' node, not a variable name (identifier)",
            line,
            col,
            ident_node.kind()
        ));
    }

    let var_name = &content[ident_node.start_byte()..ident_node.end_byte()];

    // Walk up to find the declaration node.
    let decl_node = find_declaration_node(&ident_node, &caps)?;

    // Extract the initializer expression.
    let initializer = extract_initializer(content, &decl_node)?;
    let init_text = content[initializer.start_byte()..initializer.end_byte()].to_string();

    // Find the scope node (the block/function/module containing the declaration).
    let scope_node = find_scope_node(&decl_node, &caps)
        .ok_or_else(|| "Could not find a scope containing the declaration".to_string())?;

    // Find the declaration statement (the direct child of the scope block).
    let decl_stmt = find_declaration_statement(&decl_node, &scope_node)?;

    // Walk the scope to find all references and check for reassignments.
    let refs = collect_references(content, &scope_node, var_name, &decl_node, &caps)?;

    // Decide whether to wrap the init_text in parentheses.
    // Wrap if the initializer is a binary expression, conditional, or similar compound expression
    // that could have precedence issues when substituted inline.
    let replacement = if needs_parens(&initializer) {
        format!("({})", init_text)
    } else {
        init_text.clone()
    };

    // Warn if the initializer has side effects and there are multiple references.
    let mut warnings = vec![];
    if refs.len() > 1 && has_side_effects(&initializer) {
        warnings.push(format!(
            "inlining '{}' may change evaluation count: initializer appears to have side effects and is used {} times",
            var_name, refs.len()
        ));
    }

    // Build the new file content. We apply edits from back-to-front to preserve byte offsets.
    // 1. Collect all edit sites: references (sorted by start byte desc) + declaration removal.
    // 2. Also compute the declaration line range to remove.

    let decl_stmt_start = decl_stmt.start_byte();
    let decl_stmt_end = decl_stmt.end_byte();

    // The line to remove: from start of line through the newline.
    let remove_start = line_start(content, decl_stmt_start);
    let remove_end = line_end_incl(content, decl_stmt_end);

    // Sort references by start byte descending so we can apply back-to-front.
    let mut sorted_refs = refs.clone();
    sorted_refs.sort_by(|a, b| b.cmp(a));

    let mut new_content = content.to_string();

    // Apply reference replacements first (back-to-front).
    for &ref_start in &sorted_refs {
        let ref_end = ref_start + var_name.len();
        new_content.replace_range(ref_start..ref_end, &replacement);
    }

    // Now remove the declaration line. Because all references come after the declaration
    // (scope-wise), the declaration line's byte position in new_content has shifted by
    // (replacement.len() - var_name.len()) * count_refs_after_decl. However, since
    // refs are *after* the declaration in byte position, we need to account for them.
    //
    // Actually, since refs are at higher byte offsets than the declaration, replacing them
    // back-to-front does NOT shift the declaration's byte range. The declaration comes first
    // in the file. So `remove_start`/`remove_end` are still valid in `new_content`.
    new_content.replace_range(remove_start..remove_end, "");

    let references_replaced = sorted_refs.len();

    let plan = RefactoringPlan {
        operation: "inline_variable".to_string(),
        edits: vec![PlannedEdit {
            file: file.to_path_buf(),
            original: content.to_string(),
            new_content,
            description: format!("inline variable '{}'", var_name),
        }],
        warnings,
    };

    Ok(InlineVariableOutcome {
        plan,
        name: var_name.to_string(),
        references_replaced,
    })
}

/// Find the declaration node (captured `@refactor.var_decl`) that contains the
/// given identifier as the bound name.
fn find_declaration_node<'a>(
    ident: &tree_sitter::Node<'a>,
    caps: &RefactorCaptures,
) -> Result<tree_sitter::Node<'a>, String> {
    let mut current = *ident;
    loop {
        let Some(parent) = current.parent() else {
            return Err(
                "Identifier is not inside a variable declaration — cannot inline".to_string(),
            );
        };
        if caps.is("var_decl", &parent) {
            // Verify the ident is the binding, not the initializer/RHS.
            if is_binding_ident_in_var_decl(&parent, ident) {
                return Ok(parent);
            }
            return Err("Identifier is in the initializer, not the binding pattern".to_string());
        }
        current = parent;
    }
}

/// Check whether `ident` is the *binding* identifier of a declaration node,
/// rather than something in the initializer / RHS.
///
/// Pure structural (field-name) navigation — tries the binding strategies the
/// supported grammars use, keyed on the declaration's shape, not the grammar:
/// - Rust `let_declaration`: first `identifier` child before `=`.
/// - JS/TS: a `variable_declarator` whose `name` field is the ident.
/// - Python `assignment`: the `left` field (or first identifier child).
fn is_binding_ident_in_var_decl(
    decl: &tree_sitter::Node<'_>,
    ident: &tree_sitter::Node<'_>,
) -> bool {
    // Python-style: `left` field is the binding target.
    if let Some(left) = decl.child_by_field_name("left") {
        return left.id() == ident.id();
    }

    let mut cursor = decl.walk();
    for child in decl.children(&mut cursor) {
        // JS/TS-style: the binding name lives in a variable_declarator's `name` field.
        if let Some(name_node) = child.child_by_field_name("name")
            && name_node.id() == ident.id()
        {
            return true;
        }
        // Rust-style: the binding pattern is the first `identifier` before `=`.
        if child.kind() == "=" {
            break;
        }
        if child.kind() == "identifier" && child.id() == ident.id() {
            return true;
        }
    }
    false
}

/// Extract the initializer expression node from a declaration node.
///
/// Pure structural (field-name) navigation, trying the supported grammars'
/// initializer shapes: a `right` field (Python), a `variable_declarator`'s
/// `value` field (JS/TS), or the first named node after `=` (Rust et al.).
fn extract_initializer<'a>(
    content: &str,
    decl: &tree_sitter::Node<'a>,
) -> Result<tree_sitter::Node<'a>, String> {
    // Python-style: `right` field is the value.
    if let Some(right) = decl.child_by_field_name("right") {
        return Ok(right);
    }

    // JS/TS-style: a variable_declarator with a `value` field.
    let mut cursor = decl.walk();
    for child in decl.children(&mut cursor) {
        if let Some(val) = child.child_by_field_name("value") {
            return Ok(val);
        }
    }

    // Rust-style and generic: the first named node after `=` (excluding `;`).
    let mut cursor = decl.walk();
    let mut after_eq = false;
    for child in decl.children(&mut cursor) {
        if child.kind() == "=" {
            after_eq = true;
            continue;
        }
        if after_eq && child.kind() != ";" && child.is_named() {
            return Ok(child);
        }
    }

    Err(format!(
        "Variable has no initializer — cannot inline (content: {:?})",
        &content[decl.start_byte()..decl.end_byte()]
    ))
}

/// Find the innermost scope node (captured `@refactor.scope`) that contains the
/// declaration.
fn find_scope_node<'a>(
    decl: &tree_sitter::Node<'a>,
    caps: &RefactorCaptures,
) -> Option<tree_sitter::Node<'a>> {
    let mut current = decl.parent()?;
    loop {
        if caps.is("scope", &current) {
            return Some(current);
        }
        current = current.parent()?;
    }
}

/// Find the statement node that is the direct child of scope_node and contains decl.
fn find_declaration_statement<'a>(
    decl: &tree_sitter::Node<'a>,
    scope: &tree_sitter::Node<'a>,
) -> Result<tree_sitter::Node<'a>, String> {
    let mut current = *decl;
    loop {
        let Some(parent) = current.parent() else {
            return Err("Could not find declaration statement within scope".to_string());
        };
        if parent.id() == scope.id() {
            // current is a direct child of scope.
            return Ok(current);
        }
        current = parent;
    }
}

/// Walk all nodes in `scope`, collect byte offsets of identifier nodes matching `var_name`.
///
/// Excludes the declaration node itself.
/// Returns Err if any reassignment is found.
fn collect_references(
    content: &str,
    scope: &tree_sitter::Node<'_>,
    var_name: &str,
    decl: &tree_sitter::Node<'_>,
    caps: &RefactorCaptures,
) -> Result<Vec<usize>, String> {
    let mut refs: Vec<usize> = vec![];
    let mut cursor = scope.walk();

    // We need to walk the entire subtree of scope, depth-first.
    walk_tree(&mut cursor, |node| {
        // Skip the declaration itself.
        if node.id() == decl.id() {
            return WalkAction::SkipChildren;
        }
        // Only look at identifier nodes.
        if node.kind() != "identifier" {
            return WalkAction::Continue;
        }
        let text = &content[node.start_byte()..node.end_byte()];
        if text != var_name {
            return WalkAction::Continue;
        }
        // Check if this identifier is a reassignment target.
        if is_reassignment(node, caps) {
            return WalkAction::Reassignment;
        }
        refs.push(node.start_byte());
        WalkAction::Continue
    })?;

    Ok(refs)
}

enum WalkAction {
    Continue,
    SkipChildren,
    Reassignment,
}

/// Walk the tree depth-first, calling `f` on each node.
///
/// Returns Err("cannot inline: variable is reassigned at line N") if f returns Reassignment.
fn walk_tree<F>(cursor: &mut tree_sitter::TreeCursor<'_>, mut f: F) -> Result<(), String>
where
    F: FnMut(tree_sitter::Node<'_>) -> WalkAction,
{
    loop {
        let node = cursor.node();
        match f(node) {
            WalkAction::SkipChildren => {
                // Try to go to next sibling, else go to parent's next sibling.
                if cursor.goto_next_sibling() {
                    continue;
                }
                loop {
                    if !cursor.goto_parent() {
                        return Ok(());
                    }
                    if cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            WalkAction::Reassignment => {
                let ln = node.start_position().row + 1;
                return Err(format!(
                    "cannot inline: variable is reassigned at line {}",
                    ln
                ));
            }
            WalkAction::Continue => {
                if cursor.goto_first_child() {
                    continue;
                }
                // Leaf node: go to next sibling or back up.
                if cursor.goto_next_sibling() {
                    continue;
                }
                loop {
                    if !cursor.goto_parent() {
                        return Ok(());
                    }
                    if cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
    }
}

/// Check if an identifier node is a reassignment target (not a use).
///
/// Conservative: the node's parent must be captured `@refactor.reassign` and the
/// node must be its `left` (assignment-target) field.
fn is_reassignment(node: tree_sitter::Node<'_>, caps: &RefactorCaptures) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if caps.is("reassign", &parent)
        && let Some(left) = parent.child_by_field_name("left")
    {
        return left.id() == node.id();
    }
    false
}

/// Returns true if the expression node likely needs parentheses when substituted inline.
///
/// We wrap binary expressions, conditional/ternary expressions, and logical expressions
/// to be safe. Simple literals, identifiers, and call expressions don't need wrapping.
fn needs_parens(node: &tree_sitter::Node<'_>) -> bool {
    matches!(
        node.kind(),
        "binary_expression"
            | "binary_operator"  // Python
            | "conditional_expression"
            | "ternary_expression"
            | "boolean_operator"  // Python
            | "comparison_operator"  // Python
            | "not_operator"        // Python
            | "await_expression"
            | "yield_expression"
            | "range_expression"   // Rust
            | "as_expression"      // Rust
            | "reference_expression" // Rust
    )
}

/// Heuristic: does the expression likely have side effects?
///
/// We flag function/method calls and await expressions as having potential side effects.
fn has_side_effects(node: &tree_sitter::Node<'_>) -> bool {
    match node.kind() {
        "call_expression" | "call" | "method_call_expression" | "await_expression" => true,
        _ => {
            // Recurse into children.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if has_side_effects(&child) {
                    return true;
                }
            }
            false
        }
    }
}

/// Return the byte position of the start of the line containing `pos`.
fn line_start(content: &str, pos: usize) -> usize {
    content[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// Return the byte position just past the end of the line containing `pos`.
/// Includes the trailing newline character if present.
fn line_end_incl(content: &str, pos: usize) -> usize {
    match content[pos..].find('\n') {
        Some(offset) => pos + offset + 1, // include the newline
        None => content.len(),            // last line without trailing newline
    }
}

/// Convert a 1-based line:col pair to a byte offset in `content`.
pub fn line_col_to_byte(content: &str, line: usize, col: usize) -> Option<usize> {
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

    fn ts_file() -> PathBuf {
        PathBuf::from("test.ts")
    }

    fn py_file() -> PathBuf {
        PathBuf::from("test.py")
    }

    fn js_file() -> PathBuf {
        PathBuf::from("test.js")
    }

    /// Find the 1-based line:col of the first occurrence of `needle` in `content`.
    fn find_pos(content: &str, needle: &str) -> (usize, usize) {
        let byte_pos = content
            .find(needle)
            .unwrap_or_else(|| panic!("needle {:?} not found", needle));
        let mut line = 1usize;
        let mut col = 1usize;
        for (i, ch) in content.char_indices() {
            if i == byte_pos {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    #[test]
    fn test_rust_inline_simple() {
        let content = "fn main() {\n    let x = 1 + 2;\n    println!(\"{}\", x);\n}\n";
        let (line, col) = find_pos(content, "x = 1 + 2");
        let outcome = plan_inline_variable(&rust_file(), content, line, col).unwrap();
        assert_eq!(outcome.name, "x");
        assert_eq!(outcome.references_replaced, 1);
        let new_content = &outcome.plan.edits[0].new_content;
        // Declaration removed.
        assert!(
            !new_content.contains("let x = 1 + 2"),
            "declaration should be removed, got:\n{}",
            new_content
        );
        // Reference replaced — binary expression wrapped in parens.
        assert!(
            new_content.contains("(1 + 2)"),
            "expected parens-wrapped replacement, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_rust_inline_no_references() {
        let content = "fn main() {\n    let x = 42;\n    println!(\"hello\");\n}\n";
        let (line, col) = find_pos(content, "x = 42");
        let outcome = plan_inline_variable(&rust_file(), content, line, col).unwrap();
        assert_eq!(outcome.references_replaced, 0);
        // Declaration should be removed even with 0 references.
        let new_content = &outcome.plan.edits[0].new_content;
        assert!(
            !new_content.contains("let x = 42"),
            "declaration should be removed, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_rust_inline_identifier_initializer() {
        // Identifier initializer — no parens needed.
        let content = "fn main() {\n    let x = some_val;\n    let y = x + 1;\n}\n";
        let (line, col) = find_pos(content, "x = some_val");
        let outcome = plan_inline_variable(&rust_file(), content, line, col).unwrap();
        let new_content = &outcome.plan.edits[0].new_content;
        // No parens around a bare identifier.
        assert!(
            new_content.contains("some_val + 1"),
            "expected no parens for identifier, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_rust_error_on_reassignment() {
        let content = "fn main() {\n    let mut x = 1;\n    x = 2;\n    println!(\"{}\", x);\n}\n";
        let (line, col) = find_pos(content, "x = 1");
        let result = plan_inline_variable(&rust_file(), content, line, col);
        let msg = result.err().expect("should error on reassignment");
        assert!(
            msg.contains("reassigned"),
            "error should mention reassignment, got: {}",
            msg
        );
    }

    #[test]
    fn test_typescript_inline_const() {
        let content = "function main() {\n    const x = 1 + 2;\n    console.log(x);\n}\n";
        let (line, col) = find_pos(content, "x = 1 + 2");
        let outcome = plan_inline_variable(&ts_file(), content, line, col).unwrap();
        assert_eq!(outcome.name, "x");
        assert_eq!(outcome.references_replaced, 1);
        let new_content = &outcome.plan.edits[0].new_content;
        assert!(
            !new_content.contains("const x = 1 + 2"),
            "declaration should be removed, got:\n{}",
            new_content
        );
        assert!(
            new_content.contains("(1 + 2)"),
            "expected wrapped replacement, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_javascript_inline_var() {
        let content = "function main() {\n    var x = foo();\n    return x;\n}\n";
        let (line, col) = find_pos(content, "x = foo()");
        let outcome = plan_inline_variable(&js_file(), content, line, col).unwrap();
        assert_eq!(outcome.references_replaced, 1);
        let new_content = &outcome.plan.edits[0].new_content;
        assert!(
            !new_content.contains("var x = foo()"),
            "declaration removed, got:\n{}",
            new_content
        );
        assert!(
            new_content.contains("return foo()"),
            "expected foo() inlined, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_python_inline_assignment() {
        let content = "def main():\n    x = 1 + 2\n    print(x)\n";
        let (line, col) = find_pos(content, "x = 1 + 2");
        let outcome = plan_inline_variable(&py_file(), content, line, col).unwrap();
        assert_eq!(outcome.references_replaced, 1);
        let new_content = &outcome.plan.edits[0].new_content;
        assert!(
            !new_content.contains("x = 1 + 2"),
            "declaration removed, got:\n{}",
            new_content
        );
        assert!(
            new_content.contains("print((1 + 2))"),
            "expected wrapped replacement, got:\n{}",
            new_content
        );
    }

    #[test]
    fn test_error_on_no_initializer() {
        // A `let x;` in Rust has no initializer.
        let content = "fn main() {\n    let x;\n    x = 5;\n    println!(\"{}\", x);\n}\n";
        let (line, col) = find_pos(content, "x;");
        let result = plan_inline_variable(&rust_file(), content, line, col);
        // This will error because x is reassigned, OR because there's no initializer.
        assert!(
            result.is_err(),
            "should error on missing initializer or reassignment"
        );
    }

    #[test]
    fn test_multiple_references_warns_on_side_effects() {
        let content = "fn main() {\n    let x = foo();\n    let _a = x;\n    let _b = x;\n}\n";
        let (line, col) = find_pos(content, "x = foo()");
        let outcome = plan_inline_variable(&rust_file(), content, line, col).unwrap();
        assert_eq!(outcome.references_replaced, 2);
        assert!(
            !outcome.plan.warnings.is_empty(),
            "should warn about side effects with multiple references"
        );
    }

    /// Returns true if the named external grammar can be loaded.
    fn grammar_available(name: &str) -> bool {
        normalize_languages::parsers::parser_for(name).is_some()
    }

    #[test]
    fn test_unsupported_language_returns_clean_error() {
        // Go has codegen but no `*.refactor.scm` query — the recipe must refuse
        // with a clear "does not support" message rather than fall through.
        if !grammar_available("go") {
            eprintln!("skipping: go grammar not available");
            return;
        }
        let content = "func main() {\n    x := 1 + 2\n    println(x)\n}\n";
        let (line, col) = find_pos(content, "x := 1");
        let result = plan_inline_variable(&PathBuf::from("test.go"), content, line, col);
        let msg = result.err().expect("should error for unsupported language");
        assert!(
            msg.contains("does not support") && msg.contains("refactor query"),
            "expected a clean unsupported-language error, got: {}",
            msg
        );
    }
}
