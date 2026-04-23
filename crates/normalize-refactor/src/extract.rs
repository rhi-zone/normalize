//! Extract-function recipe: lift a code selection into a named function.
//!
//! ## What it does
//!
//! Given a file, a byte range, and a new function name:
//! 1. Parses the file with tree-sitter to find the enclosing function.
//! 2. Infers free variables — identifiers referenced in the selection that are
//!    not defined within it — to form the parameter list.
//! 3. Produces two edits:
//!    (a) Replace the selected code with a call to the new function.
//!    (b) Insert the new function definition immediately after the enclosing function.
//!
//! ## Limitations
//!
//! - Free-variable inference is a best-effort heuristic. It identifies
//!   `identifier` nodes inside the selection that appear to be local bindings
//!   introduced outside the selection. It does not do full type inference.
//! - Language support depends on tree-sitter grammars. Extraction will degrade
//!   gracefully (no parameters inferred) when the grammar isn't loaded.
//! - The generated call uses positional parameters. No return value handling.

use std::path::Path;

use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::support_for_path;

use crate::{PlannedEdit, RefactoringContext, RefactoringPlan};

/// Input parameters for the extract-function recipe.
pub struct ExtractFunctionParams<'a> {
    /// Absolute path to the source file.
    pub file: &'a Path,
    /// Start byte of the code to extract (inclusive).
    pub start_byte: usize,
    /// End byte of the code to extract (exclusive).
    pub end_byte: usize,
    /// Name for the newly extracted function.
    pub new_name: &'a str,
}

/// Build an extract-function plan without touching the filesystem.
///
/// Returns a `RefactoringPlan` with at most two edits:
/// 1. Replace the selection with a call to the new function.
/// 2. Insert the new function definition after the enclosing function.
///
/// Returns an error if the file cannot be read, the byte range is invalid, or
/// no enclosing function can be found.
pub fn plan_extract_function(
    ctx: &RefactoringContext,
    params: &ExtractFunctionParams<'_>,
) -> Result<RefactoringPlan, String> {
    let content = std::fs::read_to_string(params.file)
        .map_err(|e| format!("Error reading {}: {}", params.file.display(), e))?;

    if params.start_byte >= params.end_byte {
        return Err(format!(
            "Invalid byte range: start ({}) must be less than end ({})",
            params.start_byte, params.end_byte
        ));
    }
    if params.end_byte > content.len() {
        return Err(format!(
            "Byte range end ({}) exceeds file length ({})",
            params.end_byte,
            content.len()
        ));
    }

    let selected_code = content[params.start_byte..params.end_byte].to_string();

    // Trim leading/trailing whitespace from the selection for the extracted body
    let trimmed_code = selected_code.trim().to_string();
    if trimmed_code.is_empty() {
        return Err("Selected range contains only whitespace".to_string());
    }

    // Use tree-sitter to find enclosing function and infer parameters
    let support = support_for_path(params.file);
    let grammar_name = support.map(|s| s.grammar_name());

    let (enclosing_end_byte, enclosing_indent, free_vars) = if let Some(grammar) = grammar_name {
        if let Some(tree) = parse_with_grammar(grammar, &content) {
            let root = tree.root_node();
            // Find the node overlapping the selection
            let selection_node = root
                .named_descendant_for_byte_range(params.start_byte, params.end_byte)
                .unwrap_or(root);

            // Walk up to find the nearest function-like enclosing node
            let enclosing = find_enclosing_function(selection_node, &content);

            let (end_byte, indent) = if let Some(ref enc) = enclosing {
                let end = enc.end_byte();
                // Detect indentation from the enclosing function's start line
                let line_start = content[..enc.start_byte()]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let detected_indent: String = content[line_start..enc.start_byte()]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                (end, detected_indent)
            } else {
                // Fall back to end of file, no indentation
                (content.len(), String::new())
            };

            // Infer free variables from the selection
            let vars = infer_free_variables(&tree, &content, params.start_byte, params.end_byte);

            (end_byte, indent, vars)
        } else {
            (content.len(), String::new(), vec![])
        }
    } else {
        (content.len(), String::new(), vec![])
    };

    // Build the new function definition
    let param_list = free_vars.join(", ");
    let new_function = build_function_definition(
        params.file,
        params.new_name,
        &param_list,
        &trimmed_code,
        &enclosing_indent,
        &content,
        params.start_byte,
    );

    // Build the call expression
    let call_expr = format!("{}({})", params.new_name, param_list);

    // Detect indentation of the selected code (for the replacement call)
    let selection_indent = detect_indent(&content, params.start_byte);

    // Replace selection with call
    let mut new_content = content[..params.start_byte].to_string();
    new_content.push_str(&selection_indent);
    new_content.push_str(&call_expr);
    // Preserve trailing content after the selection (keep trailing newline if present)
    let after = &content[params.end_byte..];
    new_content.push_str(after);

    // Insert new function after the enclosing function
    // We do this in the same PlannedEdit (operating on new_content) so both
    // changes are applied atomically.
    // Recalculate insert position in new_content (enclosing_end_byte may have shifted
    // if the selection was shorter/longer than the call).
    let delta: i64 =
        call_expr.len() as i64 + selection_indent.len() as i64 - selected_code.len() as i64;
    let insert_pos = ((enclosing_end_byte as i64 + delta).max(0) as usize).min(new_content.len());

    // Find end of the enclosing function's line (include trailing newline)
    let insert_pos = {
        let after_enc = &new_content[insert_pos..];
        let nl = after_enc
            .find('\n')
            .map(|i| i + 1)
            .unwrap_or(after_enc.len());
        insert_pos + nl
    };

    let mut final_content = new_content[..insert_pos].to_string();
    // Add blank line separator
    if !final_content.ends_with("\n\n") {
        if final_content.ends_with('\n') {
            final_content.push('\n');
        } else {
            final_content.push_str("\n\n");
        }
    }
    final_content.push_str(&new_function);
    if !new_function.ends_with('\n') {
        final_content.push('\n');
    }
    // Preserve remaining content
    let remainder = &new_content[insert_pos..];
    // Don't double-add blank line if remainder starts with blank line already
    if !remainder.is_empty() {
        if !final_content.ends_with("\n\n") && !remainder.starts_with('\n') {
            final_content.push('\n');
        }
        final_content.push_str(remainder);
    }

    let rel_path = params
        .file
        .strip_prefix(&ctx.root)
        .unwrap_or(params.file)
        .to_string_lossy();

    let edit = PlannedEdit {
        file: params.file.to_path_buf(),
        original: content,
        new_content: final_content,
        description: format!(
            "extract '{}' into {}",
            trimmed_code.lines().next().unwrap_or("..."),
            params.new_name
        ),
    };

    let mut warnings = vec![];
    if free_vars.is_empty() && grammar_name.is_none() {
        warnings.push(format!(
            "Grammar not available for {}; parameters not inferred",
            rel_path
        ));
    }

    Ok(RefactoringPlan {
        operation: "extract_function".to_string(),
        edits: vec![edit],
        warnings,
    })
}

/// Walk from `node` up the tree to the nearest function-like ancestor.
///
/// Returns the first ancestor whose node kind matches common function patterns
/// across languages. Falls back to the root if no function ancestor is found.
fn find_enclosing_function<'a>(
    node: tree_sitter::Node<'a>,
    _content: &str,
) -> Option<tree_sitter::Node<'a>> {
    let function_kinds = [
        // Rust
        "function_item",
        // Python
        "function_definition",
        // JavaScript / TypeScript
        "function_declaration",
        "function",
        "arrow_function",
        "method_definition",
        // Go
        "function_declaration",
        "method_declaration",
        // Java / Kotlin / C# / Scala
        "method_declaration",
        "constructor_declaration",
        "function_declaration",
        // C / C++
        "function_definition",
        // Ruby
        "method",
        "singleton_method",
        // Elixir
        "def",
        "defp",
        // Haskell
        "function",
        // Generic
        "function",
        "method",
        "closure_expression",
        "lambda",
        "lambda_expression",
    ];

    let mut current = node;
    loop {
        if function_kinds.contains(&current.kind()) {
            return Some(current);
        }
        match current.parent() {
            Some(p) => current = p,
            None => return None,
        }
    }
}

/// Detect the leading whitespace (indentation) of the line containing `byte_pos`.
fn detect_indent(content: &str, byte_pos: usize) -> String {
    // Find start of the line
    let line_start = content[..byte_pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    content[line_start..byte_pos]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect()
}

/// Build a function definition string for the extracted code.
///
/// The signature style is inferred from the file extension:
/// - Rust: `fn name(params) {\n    body\n}`
/// - Python: `def name(params):\n    body`
/// - JavaScript/TypeScript: `function name(params) {\n    body\n}`
/// - Go: `func name(params) {\n    body\n}`
/// - Default: `function name(params) {\n    body\n}`
fn build_function_definition(
    file: &Path,
    name: &str,
    params: &str,
    body: &str,
    indent: &str,
    content: &str,
    selection_start: usize,
) -> String {
    // Detect inner indentation from the selection
    let inner_indent = detect_inner_indent(body, content, selection_start);

    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();

    match ext {
        "rs" => {
            // Rust: fn name(params) { body }
            let indented_body = reindent(body, &inner_indent, &format!("{}    ", indent));
            format!(
                "{}fn {}({}) {{\n{}\n{}}}",
                indent, name, params, indented_body, indent
            )
        }
        "py" | "pyi" => {
            // Python: def name(params): body
            let indented_body = reindent(body, &inner_indent, &format!("{}    ", indent));
            format!("{}def {}({}):\n{}", indent, name, params, indented_body)
        }
        "go" => {
            // Go: func name(params) { body }
            let indented_body = reindent(body, &inner_indent, &format!("{}    ", indent));
            format!(
                "{}func {}({}) {{\n{}\n{}}}",
                indent, name, params, indented_body, indent
            )
        }
        "rb" => {
            // Ruby: def name(params) body end
            let indented_body = reindent(body, &inner_indent, &format!("{}  ", indent));
            format!(
                "{}def {}({})\n{}\n{}end",
                indent, name, params, indented_body, indent
            )
        }
        "java" | "kt" | "cs" | "cpp" | "c" | "cc" | "cxx" | "h" | "hpp" => {
            // C-like languages
            let indented_body = reindent(body, &inner_indent, &format!("{}    ", indent));
            format!(
                "{}void {}({}) {{\n{}\n{}}}",
                indent, name, params, indented_body, indent
            )
        }
        // JavaScript, TypeScript, and others default to `function` syntax
        _ => {
            let indented_body = reindent(body, &inner_indent, &format!("{}  ", indent));
            format!(
                "{}function {}({}) {{\n{}\n{}}}",
                indent, name, params, indented_body, indent
            )
        }
    }
}

/// Detect the common leading indentation of the selected body code.
fn detect_inner_indent(body: &str, _content: &str, _selection_start: usize) -> String {
    // Find the minimum indentation across non-empty lines
    body.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.chars()
                .take_while(|c| c.is_whitespace())
                .collect::<String>()
        })
        .min_by_key(|s| s.len())
        .unwrap_or_default()
}

/// Re-indent `body` by stripping `old_indent` and prepending `new_indent` to each line.
fn reindent(body: &str, old_indent: &str, new_indent: &str) -> String {
    body.lines()
        .map(|line| {
            if line.trim().is_empty() {
                line.to_string()
            } else if let Some(stripped) = line.strip_prefix(old_indent) {
                format!("{}{}", new_indent, stripped)
            } else {
                format!("{}{}", new_indent, line.trim_start())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Infer free variables in the selected byte range.
///
/// A "free variable" is an identifier used in the selection that appears to be
/// a binding introduced before the selection (not defined within it).
///
/// Strategy:
/// 1. Collect all `identifier` leaf nodes fully inside the selection.
/// 2. Find all identifier nodes in the enclosing scope that appear to be
///    let/variable bindings (simple: look for nodes whose text matches and
///    whose parent has kind suggestive of a binding).
/// 3. Return identifiers that appear in step 1 but whose definitions (in the
///    function body) start before the selection.
///
/// This is a best-effort heuristic — it will miss cases and may include noise.
/// The result is deduplicated and sorted for determinism.
fn infer_free_variables(
    tree: &tree_sitter::Tree,
    content: &str,
    start_byte: usize,
    end_byte: usize,
) -> Vec<String> {
    let root = tree.root_node();

    // Collect identifiers used inside the selection
    let used_in_selection = collect_identifiers_in_range(root, content, start_byte, end_byte);

    if used_in_selection.is_empty() {
        return vec![];
    }

    // Collect identifiers defined (bound) inside the selection — these are NOT free
    let defined_in_selection = collect_defined_names(root, content, start_byte, end_byte);

    // Collect names defined outside the selection but before it (in the enclosing scope)
    // We look from the enclosing function's start up to selection_start
    let enclosing_fn = root
        .named_descendant_for_byte_range(start_byte, end_byte)
        .and_then(|n| find_enclosing_function(n, content));

    let scope_start = enclosing_fn.map(|n| n.start_byte()).unwrap_or(0);
    let defined_before = collect_defined_names(root, content, scope_start, start_byte);

    // Free vars = used in selection AND (defined before selection OR are function params)
    // AND NOT defined within selection
    let mut free: Vec<String> = used_in_selection
        .into_iter()
        .filter(|name| !defined_in_selection.contains(name))
        .filter(|name| defined_before.contains(name))
        .collect();

    free.sort();
    free.dedup();
    free
}

/// Collect all identifier leaf text nodes fully within [start_byte, end_byte).
fn collect_identifiers_in_range(
    root: tree_sitter::Node<'_>,
    content: &str,
    start_byte: usize,
    end_byte: usize,
) -> std::collections::HashSet<String> {
    let mut result = std::collections::HashSet::new();
    collect_identifiers_rec(root, content, start_byte, end_byte, &mut result);
    result
}

fn collect_identifiers_rec(
    node: tree_sitter::Node<'_>,
    content: &str,
    start_byte: usize,
    end_byte: usize,
    out: &mut std::collections::HashSet<String>,
) {
    let ns = node.start_byte();
    let ne = node.end_byte();

    // Skip nodes entirely outside the range
    if ne <= start_byte || ns >= end_byte {
        return;
    }

    // Check if this is an identifier node fully inside the range
    if is_identifier_kind(node.kind()) && ns >= start_byte && ne <= end_byte && node.is_named() {
        let text = &content[ns..ne];
        // Filter out keywords and very short tokens
        if !text.is_empty() && !is_likely_keyword(text) {
            out.insert(text.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifiers_rec(child, content, start_byte, end_byte, out);
    }
}

/// Collect names that appear to be defined (bound) within [start_byte, end_byte).
///
/// This looks for patterns like `let x`, `x =`, `def x`, `fn x`, etc.
/// Returns a set of identifier strings.
fn collect_defined_names(
    root: tree_sitter::Node<'_>,
    content: &str,
    start_byte: usize,
    end_byte: usize,
) -> std::collections::HashSet<String> {
    let mut result = std::collections::HashSet::new();
    collect_defined_names_rec(root, content, start_byte, end_byte, &mut result);
    result
}

fn collect_defined_names_rec(
    node: tree_sitter::Node<'_>,
    content: &str,
    start_byte: usize,
    end_byte: usize,
    out: &mut std::collections::HashSet<String>,
) {
    let ns = node.start_byte();
    let ne = node.end_byte();

    // Skip nodes entirely outside the range
    if ne <= start_byte || ns >= end_byte {
        return;
    }

    let kind = node.kind();

    // Check for binding patterns
    let is_binding = matches!(
        kind,
        // Rust
        "let_declaration"
        | "parameter"
        | "closure_parameters"
        // Python
        | "augmented_assignment"
        | "named_expression"
        | "for_statement"
        | "with_statement"
        // JavaScript / TypeScript
        | "variable_declarator"
        | "lexical_declaration"
        // Go
        | "short_var_declaration"
        | "var_spec"
        // Java / Kotlin / C# / C / C++
        | "local_variable_declaration"
        | "parameter_declaration"
        // Generic (covers Python/Ruby assignment and JS variable_declaration)
        | "assignment"
        | "assignment_expression"
        | "variable_declaration"
    );

    if is_binding {
        // Extract the bound name: look for the first identifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if is_identifier_kind(child.kind()) && child.is_named() {
                let cs = child.start_byte();
                let ce = child.end_byte();
                if cs >= start_byte && ce <= end_byte {
                    let text = &content[cs..ce];
                    if !text.is_empty() && !is_likely_keyword(text) {
                        out.insert(text.to_string());
                    }
                }
                break; // Only the first identifier is the binding target (heuristic)
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_defined_names_rec(child, content, start_byte, end_byte, out);
    }
}

/// Returns true for node kinds that represent identifiers across common languages.
fn is_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier" | "name" | "variable_name" | "local_variable_identifier" | "simple_identifier"
    )
}

/// Returns true for common language keywords that shouldn't be treated as variables.
fn is_likely_keyword(text: &str) -> bool {
    matches!(
        text,
        "self"
            | "this"
            | "super"
            | "true"
            | "false"
            | "nil"
            | "null"
            | "None"
            | "True"
            | "False"
            | "undefined"
            | "void"
            | "return"
            | "break"
            | "continue"
            | "if"
            | "else"
            | "for"
            | "while"
            | "in"
            | "is"
            | "as"
            | "and"
            | "or"
            | "not"
            | "let"
            | "var"
            | "const"
            | "fn"
            | "func"
            | "def"
            | "class"
            | "struct"
            | "impl"
            | "trait"
            | "mod"
            | "use"
            | "pub"
            | "mut"
            | "ref"
            | "move"
            | "async"
            | "await"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RefactoringContext, RefactoringExecutor};
    use normalize_edit::Editor;
    use normalize_languages::GrammarLoader;

    fn make_ctx(root: &Path) -> RefactoringContext {
        RefactoringContext {
            root: root.to_path_buf(),
            editor: Editor::new(),
            index: None,
            loader: GrammarLoader::new(),
        }
    }

    #[test]
    fn extract_function_basic_python() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.py");
        let content = "def outer():\n    x = 1\n    y = 2\n    z = x + y\n    return z\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        // Select "z = x + y" line: bytes 32..42 approximately
        // Find exact bytes:
        let start = content.find("z = x + y").unwrap();
        let end = start + "z = x + y".len();

        let params = ExtractFunctionParams {
            file: &file,
            start_byte: start,
            end_byte: end,
            new_name: "compute_z",
        };

        let plan = plan_extract_function(&ctx, &params).unwrap();
        assert_eq!(plan.edits.len(), 1);
        let edit = &plan.edits[0];
        // New content should contain the call
        assert!(
            edit.new_content.contains("compute_z("),
            "Expected call in:\n{}",
            edit.new_content
        );
        // New content should contain the function definition
        assert!(
            edit.new_content.contains("def compute_z("),
            "Expected def in:\n{}",
            edit.new_content
        );
    }

    #[test]
    fn extract_function_basic_rust() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        let content = "fn outer() {\n    let x = 1;\n    let y = 2;\n    let z = x + y;\n    println!(\"{}\", z);\n}\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        let start = content.find("let z = x + y;").unwrap();
        let end = start + "let z = x + y;".len();

        let params = ExtractFunctionParams {
            file: &file,
            start_byte: start,
            end_byte: end,
            new_name: "compute_z",
        };

        let plan = plan_extract_function(&ctx, &params).unwrap();
        assert_eq!(plan.edits.len(), 1);
        let edit = &plan.edits[0];
        assert!(
            edit.new_content.contains("compute_z("),
            "Expected call in:\n{}",
            edit.new_content
        );
        assert!(
            edit.new_content.contains("fn compute_z("),
            "Expected fn def in:\n{}",
            edit.new_content
        );
    }

    #[test]
    fn extract_function_invalid_range() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.py");
        std::fs::write(&file, "x = 1\n").unwrap();

        let ctx = make_ctx(dir.path());

        // start >= end
        let params = ExtractFunctionParams {
            file: &file,
            start_byte: 5,
            end_byte: 2,
            new_name: "extracted",
        };
        assert!(plan_extract_function(&ctx, &params).is_err());

        // end > file length
        let params2 = ExtractFunctionParams {
            file: &file,
            start_byte: 0,
            end_byte: 9999,
            new_name: "extracted",
        };
        assert!(plan_extract_function(&ctx, &params2).is_err());
    }

    #[test]
    fn extract_function_dry_run_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.py");
        let content = "def outer():\n    x = 1 + 2\n    return x\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        let start = content.find("x = 1 + 2").unwrap();
        let end = start + "x = 1 + 2".len();

        let params = ExtractFunctionParams {
            file: &file,
            start_byte: start,
            end_byte: end,
            new_name: "add_nums",
        };

        let plan = plan_extract_function(&ctx, &params).unwrap();

        let executor = RefactoringExecutor {
            root: dir.path().to_path_buf(),
            dry_run: true,
            shadow_enabled: false,
            message: None,
        };

        let result = executor.apply(&plan).unwrap();
        assert!(!result.is_empty());
        // File should be unchanged on dry run
        assert_eq!(std::fs::read_to_string(&file).unwrap(), content);
    }

    #[test]
    fn extract_function_reindent() {
        assert_eq!(
            reindent("    x = 1\n    y = 2", "    ", "        "),
            "        x = 1\n        y = 2"
        );
        assert_eq!(reindent("x = 1", "", "    "), "    x = 1");
    }

    #[test]
    fn extract_function_detect_inner_indent() {
        let body = "    x = 1\n    y = 2";
        let indent = detect_inner_indent(body, "", 0);
        assert_eq!(indent, "    ");
    }
}
