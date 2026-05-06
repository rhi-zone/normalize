//! Inline-function recipe: replace a single-use function's call site with its body.
//!
//! Steps:
//! 1. Locate the function definition at the given position (line:col may point to
//!    the function name in the definition or in a call site)
//! 2. Find all call sites of this function within the same file (name-match)
//! 3. Verify the function is called exactly once (or `--force` to override)
//! 4. Substitute arguments for parameters throughout the body
//! 5. Replace the call expression with the inlined body
//! 6. Remove the function definition
//!
//! Supported languages: JavaScript, TypeScript (function declarations,
//! arrow-function const bindings). Python (`def`) and Rust (`fn`) are
//! structurally similar but less tested.
//!
//! Conservative: aborts rather than generating broken code.
//! - Multiple `return` statements → error
//! - Non-trivial control flow → error
//! - Grammar unavailable → error

use std::path::Path;

use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::support_for_path;

use crate::{PlannedEdit, RefactoringContext, RefactoringPlan};

// ── Public output type ────────────────────────────────────────────────

/// Outcome details for a planned inline-function operation.
pub struct InlineFunctionOutcome {
    pub plan: RefactoringPlan,
    pub function_name: String,
    pub call_site_line: usize,
}

// ── Entry point ───────────────────────────────────────────────────────

/// Build an inline-function plan without touching the filesystem.
///
/// `file_path` is the path of the file (absolute or relative; used for grammar
/// detection and error messages). `content` is the file's current text.
/// `line` and `col` are 1-based and point to the function name — either in the
/// definition or at a call site. `force` overrides the single-use check.
pub fn plan_inline_function(
    _ctx: &RefactoringContext,
    file_abs: &Path,
    content: &str,
    line: usize,
    col: usize,
    force: bool,
) -> Result<InlineFunctionOutcome, String> {
    // ── 1. Grammar ────────────────────────────────────────────────────
    let support = support_for_path(file_abs).ok_or_else(|| {
        let ext = file_abs
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("<unknown>");
        format!("inline-function: no language support for .{ext} files")
    })?;
    let grammar = support.grammar_name();
    let tree = parse_with_grammar(grammar, content).ok_or_else(|| {
        format!(
            "inline-function: grammar for {grammar} not loaded — run `normalize grammars install`"
        )
    })?;

    // ── 2. Resolve cursor position → function name ────────────────────
    let cursor_byte = line_col_to_byte(content, line, col)?;
    let root = tree.root_node();

    // Walk from the deepest node at the cursor position upward, looking for
    // either a function definition or a call_expression / call site.
    let function_name =
        resolve_function_name_at(&root, content, cursor_byte, grammar).ok_or_else(|| {
            format!("inline-function: no function definition or call found at {line}:{col}")
        })?;

    // ── 3. Find the function definition ──────────────────────────────
    let def = find_function_def(&root, content, &function_name, grammar).ok_or_else(|| {
        format!("inline-function: definition of '{function_name}' not found in this file")
    })?;

    // ── 4. Validate the function body ─────────────────────────────────
    let body_text = extract_body_text(content, &def)?;

    // ── 5. Find call sites ───────────────────────────────────────────
    let call_sites = find_call_sites(&root, content, &function_name, grammar);

    match call_sites.len() {
        0 => {
            return Err(format!(
                "inline-function: '{function_name}' has no call sites in this file"
            ));
        }
        1 => {}          // exactly one — proceed
        _ if force => {} // multiple but --force
        n => {
            return Err(format!(
                "inline-function: '{function_name}' is called {n} times; use --force to inline anyway (or inline the specific call manually)"
            ));
        }
    }

    let call_site = &call_sites[0];

    // ── 6. Perform substitution ───────────────────────────────────────
    let inlined = substitute_call(content, &def, call_site, &body_text)?;
    let call_site_line = call_site.line;

    // ── 7. Remove the function definition ────────────────────────────
    // `inlined` already has the call replaced; now remove the def.
    let final_content = remove_function_def(&inlined, &def, content)?;

    let plan = RefactoringPlan {
        operation: "inline-function".to_string(),
        edits: vec![PlannedEdit {
            file: file_abs.to_path_buf(),
            original: content.to_string(),
            new_content: final_content,
            description: format!("inline {function_name}"),
        }],
        warnings: vec![],
    };

    Ok(InlineFunctionOutcome {
        plan,
        function_name,
        call_site_line,
    })
}

// ── Internal types ────────────────────────────────────────────────────

/// A located function definition, with extracted parameter names and body span.
struct FunctionDef {
    /// The function name.
    name: String,
    /// Parameter names (positional, in order).
    params: Vec<String>,
    /// Byte range of the entire definition node (including any leading whitespace
    /// up to the prior newline, for clean deletion).
    def_start_byte: usize,
    def_end_byte: usize,
    /// Byte range of the function body (the `{ ... }` block, excluding braces).
    body_start_byte: usize,
    body_end_byte: usize,
}

/// A located call expression.
struct CallSite {
    /// Argument texts as they appear in source.
    args: Vec<String>,
    /// Byte range of the full call expression (e.g. `f(a, b)`).
    call_start_byte: usize,
    call_end_byte: usize,
    /// 1-based line number of the call.
    line: usize,
}

// ── Byte-position helpers ─────────────────────────────────────────────

fn line_col_to_byte(content: &str, line: usize, col: usize) -> Result<usize, String> {
    if line == 0 {
        return Err("inline-function: line is 1-based; 0 is invalid".to_string());
    }
    let mut current_line = 1usize;
    let mut line_start = 0usize;
    for (i, ch) in content.char_indices() {
        if current_line == line {
            line_start = i;
            break;
        }
        if ch == '\n' {
            current_line += 1;
        }
        if current_line > line {
            // Past end of file
            return Err(format!(
                "inline-function: line {line} is beyond end of file ({current_line} lines)"
            ));
        }
    }
    // If we reached the end without finding the line (file has exactly `line` lines
    // with no trailing newline, so the loop exits without setting line_start for the
    // last line):
    if current_line < line {
        // Check if content ends exactly at that line
        return Err(format!(
            "inline-function: line {line} is beyond end of file"
        ));
    }
    let col_offset = col.saturating_sub(1); // 1-based → 0-based
    let byte = line_start
        + col_offset.min(
            content[line_start..]
                .find('\n')
                .unwrap_or(content[line_start..].len()),
        );
    Ok(byte.min(content.len()))
}

fn byte_to_line(content: &str, byte: usize) -> usize {
    content[..byte.min(content.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        + 1
}

// ── Grammar-aware traversal helpers ──────────────────────────────────

/// Given a node at the cursor position, find the identifier name that is either
/// a function name in a definition or a callee name in a call expression.
fn resolve_function_name_at<'a>(
    root: &tree_sitter::Node<'a>,
    content: &str,
    cursor_byte: usize,
    _grammar: &str,
) -> Option<String> {
    let node = root.descendant_for_byte_range(cursor_byte, cursor_byte + 1)?;

    // Walk up trying to identify a function definition or call expression.
    let mut n = node;
    loop {
        let kind = n.kind();

        // ── Function definitions ─────────────────────────────────────
        // JS/TS: function_declaration, method_definition, arrow function via
        //        lexical_declaration (const f = (...) => ...)
        // Python: function_definition
        // Rust: function_item
        if is_function_def_kind(kind) {
            // Extract the name child
            if let Some(name_node) = find_name_child(&n, content) {
                return Some(name_node);
            }
        }

        // ── Arrow / const function bindings ──────────────────────────
        // JS/TS: `const f = (...) => ...` or `const f = function(...) {...}`
        if (kind == "lexical_declaration" || kind == "variable_declaration")
            && let Some(name) = extract_arrow_def_name(&n, content)
        {
            return Some(name);
        }

        // ── Call expressions ─────────────────────────────────────────
        // JS/TS/Python/Rust: call_expression
        if (kind == "call_expression" || kind == "call")
            && let Some(callee) = n
                .child_by_field_name("function")
                .or_else(|| n.child_by_field_name("callee"))
        {
            let callee_text = &content[callee.start_byte()..callee.end_byte()];
            // Only simple identifier calls (not method calls like a.b())
            if !callee_text.contains('.') && !callee_text.contains(':') {
                return Some(callee_text.to_string());
            }
        }

        match n.parent() {
            Some(p) if p.id() != root.id() => n = p,
            _ => break,
        }
    }
    None
}

fn is_function_def_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "function_definition"      // Python
            | "function_item"            // Rust
            | "method_definition"
            | "generator_function_declaration"
    )
}

fn is_arrow_or_func_expr_kind(kind: &str) -> bool {
    matches!(
        kind,
        "arrow_function" | "function_expression" | "generator_function"
    )
}

/// Find a parameter-list child node by kind heuristic (fallback when field names aren't available).
fn find_params_child<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    let mut c = node.walk();
    let mut found = None;
    if c.goto_first_child() {
        loop {
            let n = c.node();
            if matches!(
                n.kind(),
                "formal_parameters" | "parameters" | "parameter_list"
            ) {
                found = Some(n);
                break;
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    found
}

/// Find a body/block child node by kind heuristic.
/// For `{ ... }` bodies: find `statement_block` / `block`.
/// For arrow expression bodies: find the node after `=>`.
fn find_body_child<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    // First pass: look for explicit block kinds.
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            let n = c.node();
            if matches!(n.kind(), "statement_block" | "block" | "function_body") {
                return Some(n);
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    // Second pass: for arrow expressions, find the node after `=>`.
    let mut c = node.walk();
    let mut past_arrow = false;
    if c.goto_first_child() {
        loop {
            let n = c.node();
            if past_arrow && n.is_named() {
                return Some(n);
            }
            if n.kind() == "=>" {
                past_arrow = true;
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Extract the function name from a definition node.
fn find_name_child(node: &tree_sitter::Node<'_>, content: &str) -> Option<String> {
    // Try the `name` field first (JS/TS function_declaration, Python function_definition)
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(content[name_node.start_byte()..name_node.end_byte()].to_string());
    }
    // Rust function_item uses `name`
    None
}

/// If a `lexical_declaration` or `variable_declaration` node binds a function
/// (arrow or function expression), return the variable name.
fn extract_arrow_def_name(node: &tree_sitter::Node<'_>, content: &str) -> Option<String> {
    let mut c = node.walk();
    if c.goto_first_child() {
        loop {
            let child = c.node();
            if child.kind() == "variable_declarator"
                && let Some(name) = arrow_declarator_name(&child, content)
            {
                return Some(name);
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Extract the name from a `variable_declarator` if its value is an arrow/function expression.
/// Handles both field-based grammars and child-order-based grammars.
fn arrow_declarator_name(decl: &tree_sitter::Node<'_>, content: &str) -> Option<String> {
    // Try named field "name" + "value" first.
    let name_via_field = decl.child_by_field_name("name").or_else(|| {
        // Fallback: first named child that is an identifier.
        let mut c = decl.walk();
        let mut found = None;
        if c.goto_first_child() {
            loop {
                let n = c.node();
                if n.kind() == "identifier" {
                    found = Some(n);
                    break;
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
        found
    });
    let name_text = name_via_field.map(|n| content[n.start_byte()..n.end_byte()].to_string())?;

    // Check if the value is an arrow/function expression, using either fields or children.
    let has_func_value = decl
        .child_by_field_name("value")
        .map(|v| is_arrow_or_func_expr_kind(v.kind()))
        .unwrap_or_else(|| {
            // Fallback: scan children for an arrow/function expression.
            let mut c = decl.walk();
            let mut found = false;
            if c.goto_first_child() {
                loop {
                    let n = c.node();
                    if is_arrow_or_func_expr_kind(n.kind()) {
                        found = true;
                        break;
                    }
                    if !c.goto_next_sibling() {
                        break;
                    }
                }
            }
            found
        });

    if has_func_value {
        Some(name_text)
    } else {
        None
    }
}

// ── Function definition finder ────────────────────────────────────────

fn find_function_def(
    root: &tree_sitter::Node<'_>,
    content: &str,
    name: &str,
    _grammar: &str,
) -> Option<FunctionDef> {
    // Walk all nodes in the tree looking for function definitions named `name`.
    let mut cursor = root.walk();
    find_function_def_recursive(&mut cursor, *root, content, name)
}

fn find_function_def_recursive(
    cursor: &mut tree_sitter::TreeCursor<'_>,
    node: tree_sitter::Node<'_>,
    content: &str,
    name: &str,
) -> Option<FunctionDef> {
    let kind = node.kind();

    // Check if this node is a function definition with the right name.
    if is_function_def_kind(kind)
        && let Some(found_name) = find_name_child(&node, content)
        && found_name == name
    {
        return extract_function_def(&node, content, name, true);
    }

    // JS/TS: `const f = (...) => ...` — lexical_declaration containing an arrow_function
    if (kind == "lexical_declaration" || kind == "variable_declaration")
        && let Some(def) = try_extract_arrow_def(&node, content, name)
    {
        return Some(def);
    }

    // Recurse into children.
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if let Some(result) = find_function_def_recursive(cursor, child, content, name) {
                // Restore before returning so callers don't observe cursor side-effects.
                // (tree-sitter cursors are position-stateful but we own this cursor.)
                return Some(result);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }

    None
}

/// Try to extract an arrow-function definition from a `const f = (...) => ...` declaration.
fn try_extract_arrow_def(
    decl_node: &tree_sitter::Node<'_>,
    content: &str,
    name: &str,
) -> Option<FunctionDef> {
    // Look for a variable_declarator child with the right name binding a function.
    let mut decl_cursor = decl_node.walk();
    if decl_cursor.goto_first_child() {
        loop {
            let child = decl_cursor.node();
            if child.kind() == "variable_declarator"
                && arrow_declarator_name(&child, content).as_deref() == Some(name)
            {
                return extract_function_def(decl_node, content, name, true);
            }
            if !decl_cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Extract structured information from a function definition node.
fn extract_function_def(
    node: &tree_sitter::Node<'_>,
    content: &str,
    name: &str,
    _is_statement: bool,
) -> Option<FunctionDef> {
    // For arrow functions inside `const f = ...`, we need the body from the
    // inner arrow_function node, but the span from the outer statement.
    let (param_node, body_node) =
        if node.kind() == "lexical_declaration" || node.kind() == "variable_declaration" {
            // Find the variable_declarator → value (arrow_function or function_expression)
            let mut c = node.walk();
            let mut found_decl: Option<tree_sitter::Node<'_>> = None;
            if c.goto_first_child() {
                loop {
                    let child = c.node();
                    if child.kind() == "variable_declarator" {
                        let vname = child
                            .child_by_field_name("name")
                            .map(|n| &content[n.start_byte()..n.end_byte()]);
                        if vname == Some(name) {
                            found_decl = Some(child);
                            break;
                        }
                    }
                    if !c.goto_next_sibling() {
                        break;
                    }
                }
            }
            let decl = found_decl?;
            // The function value may be accessed via a named "value" field or as a child.
            let value = decl.child_by_field_name("value").or_else(|| {
                // Fallback: find the first arrow/function expression child.
                let mut cc = decl.walk();
                let mut found = None;
                if cc.goto_first_child() {
                    loop {
                        let n = cc.node();
                        if is_arrow_or_func_expr_kind(n.kind()) {
                            found = Some(n);
                            break;
                        }
                        if !cc.goto_next_sibling() {
                            break;
                        }
                    }
                }
                found
            })?;
            let params = value
                .child_by_field_name("parameters")
                .or_else(|| value.child_by_field_name("formal_parameters"))
                .or_else(|| find_params_child(&value))?;
            let body = value
                .child_by_field_name("body")
                .or_else(|| find_body_child(&value))?;
            (params, body)
        } else {
            // function_declaration / function_definition / function_item
            let params = node
                .child_by_field_name("parameters")
                .or_else(|| node.child_by_field_name("formal_parameters"))
                .or_else(|| find_params_child(node))?;
            let body = node
                .child_by_field_name("body")
                .or_else(|| find_body_child(node))?;
            (params, body)
        };

    let params = extract_parameter_names(&param_node, content);

    // Determine body start/end (inside the braces, if present).
    // For arrow functions with expression bodies (no braces), treat the whole
    // value as the body.
    let (body_start_byte, body_end_byte) =
        if body_node.kind() == "statement_block" || body_node.kind() == "block" {
            // Skip the opening `{` and closing `}`
            let inner_start = body_node.start_byte() + 1;
            let inner_end = body_node.end_byte() - 1;
            (inner_start, inner_end)
        } else {
            // Expression body (arrow function without braces): `(x) => x + 1`
            // The entire body is the expression.
            (body_node.start_byte(), body_node.end_byte())
        };

    // Snap def start to line start for clean deletion.
    let def_start_byte = {
        let raw = node.start_byte();
        content[..raw].rfind('\n').map(|i| i + 1).unwrap_or(0)
    };
    let def_end_byte = {
        let raw = node.end_byte();
        // Include the trailing newline if present.
        if raw < content.len() && content.as_bytes()[raw] == b'\n' {
            raw + 1
        } else {
            raw
        }
    };

    Some(FunctionDef {
        name: name.to_string(),
        params,
        def_start_byte,
        def_end_byte,
        body_start_byte,
        body_end_byte,
    })
}

/// Extract positional parameter names from a parameter list node.
fn extract_parameter_names(params_node: &tree_sitter::Node<'_>, content: &str) -> Vec<String> {
    let mut names = vec![];
    let mut c = params_node.walk();
    if c.goto_first_child() {
        loop {
            let child = c.node();
            let kind = child.kind();
            // JS/TS: identifier, required_parameter, optional_parameter, rest_parameter,
            //        assignment_pattern (default param)
            // Python: identifier, typed_parameter, default_parameter
            // Rust: pattern (identifier, typed)
            let param_name = match kind {
                "identifier" => Some(&content[child.start_byte()..child.end_byte()]),
                "required_parameter" | "optional_parameter" => child
                    .child_by_field_name("pattern")
                    .or_else(|| {
                        // Fall back to first named child that is an identifier
                        let mut cc = child.walk();
                        if cc.goto_first_child() {
                            loop {
                                let n = cc.node();
                                if n.kind() == "identifier" {
                                    return Some(n);
                                }
                                if !cc.goto_next_sibling() {
                                    break;
                                }
                            }
                        }
                        None
                    })
                    .map(|n| &content[n.start_byte()..n.end_byte()]),
                "typed_parameter" | "default_parameter" => {
                    // Python: first child is usually the name identifier
                    let mut cc = child.walk();
                    let mut found = None;
                    if cc.goto_first_child() {
                        loop {
                            let n = cc.node();
                            if n.kind() == "identifier" {
                                found = Some(&content[n.start_byte()..n.end_byte()]);
                                break;
                            }
                            if !cc.goto_next_sibling() {
                                break;
                            }
                        }
                    }
                    found
                }
                // Rust: parameter has `pattern` and `type` fields
                "parameter" => child
                    .child_by_field_name("pattern")
                    .map(|n| &content[n.start_byte()..n.end_byte()]),
                _ => None,
            };
            if let Some(n) = param_name
                && !n.is_empty()
            {
                names.push(n.to_string());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    names
}

// ── Body text extraction and validation ───────────────────────────────

/// Extract the body text from a function definition, stripping surrounding braces
/// and normalizing indentation. Returns an error if the body is too complex to
/// inline safely (e.g. multiple `return` statements).
fn extract_body_text(content: &str, def: &FunctionDef) -> Result<String, String> {
    let raw_body = &content[def.body_start_byte..def.body_end_byte];

    // Count `return` statements.  A conservative text search is good enough for
    // a first-pass safety check — if you have `return` in a string or comment,
    // this will over-count and refuse rather than under-count and generate broken code.
    // That's the correct conservative behavior.
    let return_count = count_return_statements(raw_body);
    if return_count > 1 {
        return Err(format!(
            "inline-function: '{}' has {} return statements; inlining would require control-flow analysis — aborting (too complex)",
            def.name, return_count
        ));
    }

    Ok(raw_body.to_string())
}

/// Count `return` keyword occurrences at word boundaries in `text`.
fn count_return_statements(text: &str) -> usize {
    let mut count = 0usize;
    let mut i = 0usize;
    let bytes = text.as_bytes();
    while i + 6 <= bytes.len() {
        if &bytes[i..i + 6] == b"return" {
            // Check word boundaries.
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let after = bytes.get(i + 6).copied();
            let after_ok = after.is_none_or(|b| !b.is_ascii_alphanumeric() && b != b'_');
            if before_ok && after_ok {
                count += 1;
            }
        }
        i += 1;
    }
    count
}

// ── Call site finder ──────────────────────────────────────────────────

fn find_call_sites(
    root: &tree_sitter::Node<'_>,
    content: &str,
    name: &str,
    _grammar: &str,
) -> Vec<CallSite> {
    let mut sites = vec![];
    let mut cursor = root.walk();
    find_call_sites_recursive(&mut cursor, *root, content, name, &mut sites);
    sites
}

fn find_call_sites_recursive(
    cursor: &mut tree_sitter::TreeCursor<'_>,
    node: tree_sitter::Node<'_>,
    content: &str,
    name: &str,
    sites: &mut Vec<CallSite>,
) {
    let kind = node.kind();

    if kind == "call_expression" || kind == "call" {
        // Check callee is our function name (simple identifier, not a.b or a::b).
        let callee = node
            .child_by_field_name("function")
            .or_else(|| node.child_by_field_name("callee"));
        if let Some(callee_node) = callee {
            let callee_text = &content[callee_node.start_byte()..callee_node.end_byte()];
            if callee_text == name {
                // Extract arguments.
                let args = extract_call_args(&node, content);
                let line = byte_to_line(content, node.start_byte());

                sites.push(CallSite {
                    args,
                    call_start_byte: node.start_byte(),
                    call_end_byte: node.end_byte(),
                    line,
                });
            }
        }
    }

    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            find_call_sites_recursive(cursor, child, content, name, sites);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

/// Extract argument texts from a call_expression node.
fn extract_call_args(call_node: &tree_sitter::Node<'_>, content: &str) -> Vec<String> {
    let mut args = vec![];
    let args_node = call_node.child_by_field_name("arguments").or_else(|| {
        // Fallback: look for argument_list child (Python)
        let mut c = call_node.walk();
        let mut found = None;
        if c.goto_first_child() {
            loop {
                let n = c.node();
                if matches!(n.kind(), "argument_list" | "arguments") {
                    found = Some(n);
                    break;
                }
                if !c.goto_next_sibling() {
                    break;
                }
            }
        }
        found
    });

    let Some(args_node) = args_node else {
        return args;
    };

    let mut c = args_node.walk();
    if c.goto_first_child() {
        loop {
            let child = c.node();
            let kind = child.kind();
            // Skip punctuation nodes (`,`, `(`, `)`)
            if kind != "," && kind != "(" && kind != ")" && child.is_named() {
                args.push(content[child.start_byte()..child.end_byte()].to_string());
            }
            if !c.goto_next_sibling() {
                break;
            }
        }
    }
    args
}

// ── Substitution ──────────────────────────────────────────────────────

/// Replace the call site with the inlined function body.
///
/// Steps:
/// 1. Strip the body of leading/trailing whitespace
/// 2. Replace each parameter name with the corresponding argument
/// 3. Strip the `return` keyword (if present) when the call is in expression position
/// 4. Replace the call span in `content`
fn substitute_call(
    content: &str,
    def: &FunctionDef,
    call: &CallSite,
    body_text: &str,
) -> Result<String, String> {
    // Check argument count matches parameter count.
    if call.args.len() != def.params.len() {
        return Err(format!(
            "inline-function: '{}' expects {} arguments but call site provides {} — aborting",
            def.name,
            def.params.len(),
            call.args.len()
        ));
    }

    // ── Trim the body ─────────────────────────────────────────────────
    let trimmed = body_text.trim();

    // ── Strip `return` if present ─────────────────────────────────────
    let stripped = strip_single_return(trimmed);

    // ── Substitute parameters → arguments ────────────────────────────
    let mut result = stripped.to_string();
    for (param, arg) in def.params.iter().zip(call.args.iter()) {
        result = normalize_edit::replace_all_words(&result, param, arg);
    }

    // ── Determine what replaces the call site in `content` ────────────
    //
    // If the call is the sole expression in an expression_statement, we need to
    // handle whether to keep the semicolon / statement boundary. In the simple
    // case we just replace the call expression bytes with the inlined body.
    //
    // If the function body contained a statement block, we need to decide whether
    // to emit the block inline or unwrap it. For now: if the call is a statement
    // *and* the body looks like a block (contains `;`), keep it as a block.
    // Otherwise, unwrap to an expression.
    let replacement = result.trim().to_string();

    // Build new content: replace the call bytes with the replacement.
    let mut new_content = String::new();
    new_content.push_str(&content[..call.call_start_byte]);
    new_content.push_str(&replacement);
    new_content.push_str(&content[call.call_end_byte..]);

    Ok(new_content)
}

/// Strip a leading `return ` from an expression if present (single `return`).
fn strip_single_return(s: &str) -> &str {
    let s = s.trim_start();
    if let Some(rest) = s.strip_prefix("return") {
        // Must be followed by whitespace or end-of-string.
        let after = rest.trim_start_matches([' ', '\t']);
        // Strip trailing semicolon if the body was just `return expr;`
        after.strip_suffix(';').unwrap_or(after).trim()
    } else {
        s
    }
}

// ── Definition removal ────────────────────────────────────────────────

/// Remove the function definition from `inlined` (which already has the call replaced).
///
/// Because the edit we already applied (call replacement) may have shifted byte offsets,
/// we re-locate the function definition by name in `inlined` using a text-based approach.
///
/// `original` is the original file content (used to know the original def span).
fn remove_function_def(inlined: &str, def: &FunctionDef, original: &str) -> Result<String, String> {
    // The byte delta introduced by the call replacement.
    // original length - (call_site length) + replacement length
    // We don't have the call site bytes here, but we can use a tree-sitter re-parse
    // of `inlined` to re-find the definition.
    //
    // For simplicity: use a grammar-agnostic approach — find the def by locating
    // the whole-word name on the def's original line in the new content, then
    // use the Editor's find_symbol.  But since we don't have a path here, we use
    // the editor's text-only utility.
    //
    // Actually, the cleanest approach: re-parse `inlined` to find the def.
    // Since we don't have the path or grammar here (they're not in FunctionDef),
    // we use the original line number adjusted for any inserted/removed content
    // above the definition.
    //
    // However, the simplest correct approach is: the call site and the definition
    // are in different parts of the file. If the call site is AFTER the definition,
    // the definition bytes haven't moved; we can delete them directly from `inlined`.
    // If the call site is BEFORE the definition, the definition bytes have shifted.
    //
    // We know def_start_byte from the original content.
    // The call replaced `call_start_byte..call_end_byte` with `replacement`.
    // The shift in bytes = replacement.len() - (call_end_byte - call_start_byte).
    //
    // But we don't have call_start/end here. Let's take the safe path:
    // diff the original and inlined to compute the shift, then adjust.
    //
    // Simplest path: find the function definition text in `inlined` by searching
    // for the original definition text (which is unchanged since the call was elsewhere).

    let orig_def_text = &original[def.def_start_byte..def.def_end_byte];

    // Find the definition in `inlined` by exact text match.
    if let Some(pos) = inlined.find(orig_def_text) {
        let mut result = String::new();
        result.push_str(&inlined[..pos]);
        result.push_str(&inlined[pos + orig_def_text.len()..]);
        // Clean up any double-blank lines introduced by the deletion.
        Ok(collapse_triple_newlines(result))
    } else {
        // Fallback: try to delete by adjusted byte offset.
        // Compute byte shift: compare lengths before/after the definition.
        // This handles the case where the call was in the def text itself (recursive),
        // which is already blocked by our single-use check — but be safe.
        Err(format!(
            "inline-function: could not locate definition of '{}' in modified content — aborting",
            def.name
        ))
    }
}

/// Collapse three or more consecutive newlines into two (one blank line).
fn collapse_triple_newlines(s: String) -> String {
    let mut result = s;
    loop {
        let before = result.len();
        result = result.replace("\n\n\n", "\n\n");
        if result.len() == before {
            break;
        }
    }
    result
}
