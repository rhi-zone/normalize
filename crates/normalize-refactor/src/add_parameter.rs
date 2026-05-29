//! Add-parameter recipe: insert a new parameter into a function signature and update all call sites.
//!
//! Algorithm:
//! 1. Parse `file` with tree-sitter; locate the function declaration named `function_name`.
//! 2. Find the parameter list node; determine insertion byte position.
//! 3. Build the parameter text for the definition (name + optional type annotation).
//! 4. Find all call sites via the facts index (`find_callers`). Falls back to text
//!    search if the index is unavailable, with a warning.
//! 5. At each call site: find the argument list; insert `default_value` at `position`.
//! 6. Return a `RefactoringPlan` with one `PlannedEdit` per affected file.
//!
//! Language support is driven entirely by the `*.refactor.scm` query (structural
//! classification) plus the `RefactorCodeGen` trait (parameter formatting). A
//! language with neither yields a clear "not supported" error rather than a
//! Rust-shaped default.

use std::collections::HashMap;
use std::path::Path;

use normalize_languages::parsers::parse_with_grammar;
use normalize_languages::support_for_path;

use crate::refactor_query::RefactorCaptures;
use crate::{PlannedEdit, RefactoringContext, RefactoringPlan};

/// Outcome of a successful add-parameter plan.
pub struct AddParameterOutcome {
    pub plan: RefactoringPlan,
    /// Number of call sites updated.
    pub call_sites_updated: usize,
}

/// Build an add-parameter plan without touching the filesystem.
///
/// `file` is the path to the file containing the function definition.
/// `function_name` is the name of the function to modify.
/// `param_name` is the name of the new parameter.
/// `param_type` is the optional type annotation (for typed languages).
/// `default_value` is the value to insert at call sites.
/// `position` is the 0-based index to insert at; `None` means last.
pub async fn plan_add_parameter(
    ctx: &RefactoringContext,
    def_rel_path: &str,
    function_name: &str,
    param_name: &str,
    param_type: Option<&str>,
    default_value: &str,
    position: Option<usize>,
) -> Result<AddParameterOutcome, String> {
    let def_abs_path = ctx.root.join(def_rel_path);
    let def_content = std::fs::read_to_string(&def_abs_path)
        .map_err(|e| format!("Error reading {}: {}", def_rel_path, e))?;

    // 1. Parse definition file and locate the parameter list.
    let def_edit = plan_add_param_in_definition(
        &def_abs_path,
        &def_content,
        function_name,
        param_name,
        param_type,
        position,
    )?;

    let mut edits: Vec<PlannedEdit> = vec![def_edit];
    let mut warnings: Vec<String> = vec![];

    // 2. Find call sites via the index.
    let refs = crate::actions::find_references(ctx, function_name, def_rel_path).await;

    if ctx.index.is_none() {
        warnings.push(
            "Index not available; only updated definition file. \
             Run `normalize structure rebuild` to enable call-site updates."
                .to_string(),
        );
    }

    // 3. Group callers by file.
    let mut callers_by_file: HashMap<String, Vec<usize>> = HashMap::new();
    for caller in &refs.callers {
        callers_by_file
            .entry(caller.file.clone())
            .or_default()
            .push(caller.line);
    }

    // 4. Update each call site file.
    let mut call_sites_updated = 0usize;
    for (rel_path, call_lines) in &callers_by_file {
        let abs_path = ctx.root.join(rel_path);
        let content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => {
                warnings.push(format!("Could not read caller file: {}", rel_path));
                continue;
            }
        };

        match plan_add_arg_in_file(
            &abs_path,
            &content,
            function_name,
            call_lines,
            default_value,
            position,
        ) {
            Ok(Some(edit)) => {
                call_sites_updated += call_lines.len();
                // If the definition and caller are the same file, merge edits.
                if abs_path == def_abs_path {
                    // Replace the definition edit with a merged one.
                    let merged = merge_edits(&edits[0], &edit)?;
                    edits[0] = merged;
                } else {
                    edits.push(edit);
                }
            }
            Ok(None) => {
                // No call sites matched on those lines — stale index data.
            }
            Err(e) => {
                warnings.push(format!("Could not update {}: {}", rel_path, e));
            }
        }
    }

    Ok(AddParameterOutcome {
        plan: RefactoringPlan {
            operation: "add_parameter".to_string(),
            edits,
            warnings,
        },
        call_sites_updated,
    })
}

// ── Definition editing ────────────────────────────────────────────────────────

/// Build a `PlannedEdit` inserting the new parameter into the function definition.
fn plan_add_param_in_definition(
    file: &Path,
    content: &str,
    function_name: &str,
    param_name: &str,
    param_type: Option<&str>,
    position: Option<usize>,
) -> Result<PlannedEdit, String> {
    let support = support_for_path(file)
        .ok_or_else(|| format!("No language support for {}", file.display()))?;
    let grammar = support.grammar_name();

    let tree = parse_with_grammar(grammar, content).ok_or_else(|| {
        format!(
            "Grammar '{}' not available — install grammars with `normalize grammars install`",
            grammar
        )
    })?;

    let cg = support.as_refactor_codegen().ok_or_else(|| {
        format!(
            "add-parameter does not support language {} (no code generation implemented)",
            support.name()
        )
    })?;

    let caps = RefactorCaptures::load(grammar, tree.root_node(), content).ok_or_else(|| {
        format!(
            "add-parameter does not support language {} (no refactor query)",
            support.name()
        )
    })?;

    let params_range = find_param_list(&tree.root_node(), content, &caps, function_name)
        .ok_or_else(|| {
            format!(
                "Function '{}' not found in {}",
                function_name,
                file.display()
            )
        })?;

    let param_text = cg.format_param(param_name, param_type);
    let new_content = insert_into_list(
        content,
        &params_range,
        &param_text,
        position,
        ListKind::Params,
    );

    Ok(PlannedEdit {
        file: file.to_path_buf(),
        original: content.to_string(),
        new_content,
        description: format!("add parameter '{}' to '{}'", param_name, function_name),
    })
}

// ── Call-site editing ─────────────────────────────────────────────────────────

/// Build a `PlannedEdit` inserting `default_value` into every call to `function_name`
/// on the listed lines.
fn plan_add_arg_in_file(
    file: &Path,
    content: &str,
    function_name: &str,
    call_lines: &[usize],
    default_value: &str,
    position: Option<usize>,
) -> Result<Option<PlannedEdit>, String> {
    let support = support_for_path(file)
        .ok_or_else(|| format!("No language support for {}", file.display()))?;
    let grammar = support.grammar_name();

    let tree = parse_with_grammar(grammar, content).ok_or_else(|| {
        format!(
            "Grammar '{}' not available — install grammars with `normalize grammars install`",
            grammar
        )
    })?;

    let caps = RefactorCaptures::load(grammar, tree.root_node(), content).ok_or_else(|| {
        format!(
            "add-parameter does not support language {} (no refactor query)",
            support.name()
        )
    })?;

    // Collect all argument list ranges for calls to `function_name` on the given lines.
    let ranges = find_call_arg_lists(&tree.root_node(), content, &caps, function_name, call_lines);

    if ranges.is_empty() {
        return Ok(None);
    }

    // Apply edits from last to first so byte offsets stay valid.
    let mut sorted = ranges;
    sorted.sort_by_key(|b| std::cmp::Reverse(b.open_paren));

    let mut new_content = content.to_string();
    for r in &sorted {
        let chunk = insert_into_list(&new_content, r, default_value, position, ListKind::Args);
        new_content = chunk;
    }

    Ok(Some(PlannedEdit {
        file: file.to_path_buf(),
        original: content.to_string(),
        new_content,
        description: format!(
            "add argument '{}' to calls of '{}'",
            default_value, function_name
        ),
    }))
}

// ── List manipulation ─────────────────────────────────────────────────────────

enum ListKind {
    Params,
    Args,
}

/// A parameter list or argument list — just the open-paren byte offset, the close-paren
/// byte offset, and the (sorted) byte offsets of the commas between items.
struct ListRange {
    /// Byte offset of `(`.
    open_paren: usize,
    /// Byte offset of `)`.
    close_paren: usize,
    /// Byte offsets immediately after each `,` separator (i.e. where item N+1 starts).
    comma_positions: Vec<usize>,
    /// Number of existing items.
    item_count: usize,
}

/// Insert `text` at the `position`-th slot (0-based; None = last) in the list described
/// by `range`.
fn insert_into_list(
    content: &str,
    range: &ListRange,
    text: &str,
    position: Option<usize>,
    kind: ListKind,
) -> String {
    let separator = match kind {
        ListKind::Params => ", ",
        ListKind::Args => ", ",
    };

    if range.item_count == 0 {
        // Empty list → just insert.
        let insert_at = range.open_paren + 1;
        let mut out = content.to_string();
        out.insert_str(insert_at, text);
        return out;
    }

    let pos = position.unwrap_or(range.item_count); // default: append

    if pos == 0 {
        // Insert before first item.
        let insert_at = range.open_paren + 1;
        let mut out = content.to_string();
        out.insert_str(insert_at, &format!("{}{}", text, separator));
        return out;
    }

    if pos >= range.item_count {
        // Append after last item.
        let insert_at = range.close_paren;
        let mut out = content.to_string();
        out.insert_str(insert_at, &format!("{}{}", separator, text));
        return out;
    }

    // Insert before the item at `pos` (i.e. after the (pos-1)th comma).
    let after_comma = range.comma_positions[pos - 1];
    // Skip whitespace after comma.
    let ws = content[after_comma..].len() - content[after_comma..].trim_start().len();
    let insert_at = after_comma + ws;
    let mut out = content.to_string();
    out.insert_str(insert_at, &format!("{}{}", text, separator));
    out
}

// ── Tree-sitter traversal ─────────────────────────────────────────────────────

/// Walk the tree depth-first, calling `f` on each node.
fn walk_tree(node: tree_sitter::Node<'_>, f: &mut impl FnMut(tree_sitter::Node<'_>)) {
    f(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree(child, f);
    }
}

/// Find the parameter list range for a function named `name`.
fn find_param_list(
    root: &tree_sitter::Node<'_>,
    content: &str,
    caps: &RefactorCaptures,
    name: &str,
) -> Option<ListRange> {
    let mut result: Option<ListRange> = None;
    walk_tree(*root, &mut |node| {
        if result.is_some() {
            return;
        }
        if !caps.is("function_def", &node) {
            return;
        }
        // Check if this function's name matches.
        if !function_name_matches(&node, content, name) {
            return;
        }
        // Find the parameter list child.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if caps.is("param_list", &child) {
                result = Some(list_range_from_node(&child, content));
                break;
            }
        }
    });
    result
}

/// Find argument list ranges for calls to `function_name` on the given (1-based) lines.
fn find_call_arg_lists(
    root: &tree_sitter::Node<'_>,
    content: &str,
    caps: &RefactorCaptures,
    function_name: &str,
    call_lines: &[usize],
) -> Vec<ListRange> {
    let mut results = vec![];
    walk_tree(*root, &mut |node| {
        if !caps.is("call", &node) {
            return;
        }
        // The call node's 1-based start line.
        let node_line = node.start_position().row + 1;
        if !call_lines.contains(&node_line) {
            return;
        }
        // Check that this is a call to `function_name`.
        if !call_matches_name(&node, content, function_name) {
            return;
        }
        // Find the argument list.
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if caps.is("arg_list", &child) {
                results.push(list_range_from_node(&child, content));
                break;
            }
        }
    });
    results
}

/// Build a `ListRange` from a parameter-list or argument-list node.
fn list_range_from_node(node: &tree_sitter::Node<'_>, content: &str) -> ListRange {
    let open_paren = node.start_byte();
    let close_paren = node.end_byte().saturating_sub(1);

    // Count named items and find comma positions.
    let mut comma_positions: Vec<usize> = vec![];
    let mut item_count = 0usize;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "(" | ")" => {}
            "," => {
                comma_positions.push(child.end_byte());
            }
            _ if !child.kind().starts_with('"') => {
                // Named node → an actual item.
                item_count += 1;
            }
            _ => {}
        }
        let _ = content; // not needed at this step
    }

    ListRange {
        open_paren,
        close_paren,
        comma_positions,
        item_count,
    }
}

// ── Name-extraction helpers (cross-language identifier-kind union) ─────────────

/// Check whether a function node's name identifier matches `name`.
fn function_name_matches(node: &tree_sitter::Node<'_>, content: &str, name: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "property_identifier" {
            let text = &content[child.start_byte()..child.end_byte()];
            return text == name;
        }
    }
    false
}

/// Check whether a call node refers to `function_name`.
///
/// We look at the first "function" child of the call (the callee) and check for an
/// `identifier` or `field_expression`/`member_expression` whose tail is `function_name`.
fn call_matches_name(node: &tree_sitter::Node<'_>, content: &str, name: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" || kind == "property_identifier" {
            let text = &content[child.start_byte()..child.end_byte()];
            return text == name;
        }
        // method call: obj.method(...)
        if kind == "field_expression" || kind == "member_expression" {
            let mut inner = child.walk();
            for ic in child.children(&mut inner) {
                if ic.kind() == "field_identifier"
                    || ic.kind() == "property_identifier"
                    || ic.kind() == "identifier"
                {
                    let text = &content[ic.start_byte()..ic.end_byte()];
                    if text == name {
                        return true;
                    }
                }
            }
        }
        // attribute call in Python: A.method() uses attribute node
        if kind == "attribute" {
            let mut inner = child.walk();
            for ic in child.children(&mut inner) {
                if ic.kind() == "identifier" {
                    let text = &content[ic.start_byte()..ic.end_byte()];
                    if text == name {
                        return true;
                    }
                }
            }
        }
    }
    false
}

// ── Merge helper ──────────────────────────────────────────────────────────────

/// Merge two `PlannedEdit`s for the same file: apply the second edit's transformation
/// on top of the first edit's `new_content`.
fn merge_edits(first: &PlannedEdit, second: &PlannedEdit) -> Result<PlannedEdit, String> {
    if first.file != second.file {
        return Err(format!(
            "Cannot merge edits for different files: {} vs {}",
            first.file.display(),
            second.file.display()
        ));
    }
    // The second edit was computed from `original` content; we need to re-apply it
    // on top of `first.new_content`. Since both operate on distinct byte ranges
    // (signature vs call site) in the same file, we apply the second diff as a
    // string replacement: find what changed from second.original → second.new_content
    // and apply the same replacement to first.new_content.
    //
    // Simple approach: the second edit adds text at a specific byte offset. Because
    // the definition edit is at the signature (earlier in the file for typical layout
    // than calls to itself), we apply both edits on the original and take the
    // result. Both edits are independent insertions at different byte offsets.
    //
    // If the definition is *after* the self-call (unusual but possible), the byte
    // offsets shift. We handle this by re-applying both edits from scratch on the
    // original content.
    let original = &first.original;
    // Find definition insertion: diff first.original → first.new_content
    // Find arg insertion: diff second.original → second.new_content
    // Both diffs are single string insertions. Apply them both to `original`,
    // adjusting offsets.
    let (def_pos, def_text) = extract_insertion(original, &first.new_content)?;
    let (arg_pos, arg_text) = extract_insertion(original, &second.new_content)?;

    let mut new_content = original.clone();
    // Apply in reverse order so offsets stay valid.
    if def_pos >= arg_pos {
        new_content.insert_str(def_pos, &def_text);
        new_content.insert_str(arg_pos, &arg_text);
    } else {
        new_content.insert_str(arg_pos, &arg_text);
        new_content.insert_str(def_pos, &def_text);
    }

    Ok(PlannedEdit {
        file: first.file.clone(),
        original: original.clone(),
        new_content,
        description: format!("{} + {}", first.description, second.description),
    })
}

/// Extract the single insertion made from `original` → `new_content`.
/// Returns `(byte_offset, inserted_text)`.
/// If the diff is not a simple insertion, returns an error.
fn extract_insertion(original: &str, new_content: &str) -> Result<(usize, String), String> {
    // Find the first differing byte.
    let orig_bytes = original.as_bytes();
    let new_bytes = new_content.as_bytes();

    // Both strings share a common prefix.
    let prefix_len = orig_bytes
        .iter()
        .zip(new_bytes.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Both strings share a common suffix (reading from the end).
    let orig_tail = &orig_bytes[prefix_len..];
    let new_tail = &new_bytes[prefix_len..];
    let suffix_len = orig_tail
        .iter()
        .rev()
        .zip(new_tail.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    if orig_tail.len() != suffix_len {
        return Err(format!(
            "merge_edits: expected a pure insertion but found deletion of {} bytes at offset {}",
            orig_tail.len() - suffix_len,
            prefix_len
        ));
    }

    let inserted_len = new_tail.len() - suffix_len;
    let inserted = &new_content[prefix_len..prefix_len + inserted_len];
    Ok((prefix_len, inserted.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use normalize_edit::Editor;

    fn make_ctx(root: &Path) -> RefactoringContext {
        RefactoringContext {
            root: root.to_path_buf(),
            editor: Editor::new(),
            index: None,
            loader: normalize_languages::GrammarLoader::new(),
        }
    }

    fn grammar_available(name: &str) -> bool {
        normalize_languages::parsers::parser_for(name).is_some()
    }

    // ── Rust ──────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn rust_add_param_no_callers() {
        if !grammar_available("rust") {
            eprintln!("skipping: rust grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        let content = "fn my_func(a: i32) -> bool {\n    true\n}\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        let result = plan_add_parameter(
            &ctx,
            "test.rs",
            "my_func",
            "b",
            Some("String"),
            "String::new()",
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.call_sites_updated, 0);
        let edit = &result.plan.edits[0];
        assert!(
            edit.new_content.contains("b: String"),
            "expected 'b: String' in: {}",
            edit.new_content
        );
        assert!(
            edit.new_content.contains("a: i32"),
            "expected 'a: i32' still present"
        );
        // Warning about missing index.
        assert!(!result.plan.warnings.is_empty());
    }

    #[tokio::test]
    async fn rust_add_param_at_position_zero() {
        if !grammar_available("rust") {
            eprintln!("skipping: rust grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        let content = "fn my_func(a: i32) -> bool {\n    true\n}\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        let result = plan_add_parameter(
            &ctx,
            "test.rs",
            "my_func",
            "b",
            Some("String"),
            "String::new()",
            Some(0),
        )
        .await
        .unwrap();

        let edit = &result.plan.edits[0];
        // b: String should come before a: i32
        let b_pos = edit.new_content.find("b: String").unwrap();
        let a_pos = edit.new_content.find("a: i32").unwrap();
        assert!(b_pos < a_pos, "b should come before a");
    }

    #[tokio::test]
    async fn rust_empty_param_list() {
        if !grammar_available("rust") {
            eprintln!("skipping: rust grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        let content = "fn my_func() -> bool {\n    true\n}\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        let result = plan_add_parameter(&ctx, "test.rs", "my_func", "x", Some("i32"), "0", None)
            .await
            .unwrap();

        let edit = &result.plan.edits[0];
        assert!(
            edit.new_content.contains("fn my_func(x: i32)"),
            "got: {}",
            edit.new_content
        );
    }

    // ── Python ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn python_add_param_no_callers() {
        if !grammar_available("python") {
            eprintln!("skipping: python grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.py");
        let content = "def my_func(a, b):\n    return True\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        let result = plan_add_parameter(&ctx, "test.py", "my_func", "c", None, "None", None)
            .await
            .unwrap();

        let edit = &result.plan.edits[0];
        assert!(
            edit.new_content.contains(", c)"),
            "expected ', c)' in: {}",
            edit.new_content
        );
    }

    // ── TypeScript ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn typescript_add_param_with_type() {
        if !grammar_available("typescript") {
            eprintln!("skipping: typescript grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.ts");
        let content = "function myFunc(a: number, b: string): boolean {\n    return true;\n}\n";
        std::fs::write(&file, content).unwrap();

        let ctx = make_ctx(dir.path());
        let result = plan_add_parameter(
            &ctx,
            "test.ts",
            "myFunc",
            "c",
            Some("boolean"),
            "false",
            None,
        )
        .await
        .unwrap();

        let edit = &result.plan.edits[0];
        assert!(
            edit.new_content.contains("c: boolean"),
            "expected 'c: boolean' in: {}",
            edit.new_content
        );
    }

    // ── extract_insertion ─────────────────────────────────────────────────────

    #[test]
    fn extract_insertion_middle() {
        let original = "fn f(a: i32) {}";
        let new = "fn f(a: i32, b: String) {}";
        let (pos, text) = extract_insertion(original, new).unwrap();
        // Common prefix is "fn f(a: i32" (11 bytes); insertion is ", b: String"
        assert_eq!(pos, 11);
        assert_eq!(text, ", b: String");
    }

    #[test]
    fn extract_insertion_front() {
        let original = "fn f(a: i32) {}";
        let new = "fn f(b: String, a: i32) {}";
        let (pos, text) = extract_insertion(original, new).unwrap();
        // Common prefix is "fn f(" (5 bytes); insertion is "b: String, "
        assert_eq!(pos, 5);
        assert_eq!(text, "b: String, ");
    }

    // ── PathBuf helper ────────────────────────────────────────────────────────

    #[test]
    fn function_not_found_returns_err() {
        if !grammar_available("rust") {
            eprintln!("skipping: rust grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "fn other() {}\n").unwrap();

        let res =
            plan_add_param_in_definition(&file, "fn other() {}\n", "nonexistent", "x", None, None);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert!(
            err.contains("not found"),
            "expected 'not found' in: {}",
            err
        );
    }

    #[test]
    fn unsupported_language_returns_clean_error() {
        // Nix has no `*.refactor.scm` query — add-parameter must refuse with a
        // clear "does not support" message rather than fall through.
        if !grammar_available("nix") {
            eprintln!("skipping: nix grammar not available");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.nix");
        let content = "myFunc = a: a + 1;\n";
        std::fs::write(&file, content).unwrap();

        let res = plan_add_param_in_definition(&file, content, "myFunc", "b", Some("string"), None);
        let err = res.err().expect("should error for unsupported language");
        assert!(
            err.contains("does not support"),
            "expected a clean unsupported-language error, got: {}",
            err
        );
    }
}
