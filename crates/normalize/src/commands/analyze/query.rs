//! Tree-sitter and ast-grep query support for code search.
//!
//! Supports two pattern syntaxes (auto-detected):
//! - Tree-sitter S-expression: `(call_expression function: (identifier) @fn)`
//! - ast-grep pattern: `$FN($ARGS)` (more human-friendly)

use crate::filter::Filter;
use crate::output::OutputFormat;
use crate::parsers::grammar_loader;
use crate::tree::highlight_source;
use normalize_languages::ast_grep::DynLang;
use normalize_languages::support_for_path;
use normalize_syntax_rules::evaluate_predicates;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

/// Detect if pattern is a tree-sitter S-expression (starts with `(`)
/// or an ast-grep pattern (anything else).
fn is_sexp_pattern(pattern: &str) -> bool {
    pattern.trim_start().starts_with('(')
}

/// How to preview a match result.
enum PreviewKind {
    /// Show full text (small match or --show-source)
    Full,
    /// Show method signatures for containers (impl, class, trait)
    Skeleton,
    /// Show structural elements for functions (control flow, bindings)
    Structural,
    /// Show first N lines for other nodes
    Truncated,
}

/// Node kinds that are "containers" - should show skeleton view instead of first N lines.
const CONTAINER_KINDS: &[&str] = &[
    // Rust
    "impl_item",
    "trait_item",
    // JS/TS
    "class_declaration",
    "class",
    "interface_declaration",
    // Python
    "class_definition",
    // Go
    "type_declaration",
];

/// Node kinds that are functions - should show structural view.
const FUNCTION_KINDS: &[&str] = &[
    // Rust
    "function_item",
    // JS/TS
    "function_declaration",
    "method_definition",
    "arrow_function",
    // Python
    "function_definition",
    // Go
    "function_declaration",
    "method_declaration",
];

/// Check if a line looks like a method/function signature.
fn is_signature_line(line: &str, grammar: &str) -> bool {
    let trimmed = line.trim();
    match grammar {
        "rust" => {
            trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub(crate) fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("type ")
                || trimmed.starts_with("const ")
        }
        "javascript" | "typescript" | "tsx" => {
            // method(, async method(, get prop(, set prop(
            (trimmed.contains('(')
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("if")
                && !trimmed.starts_with("for")
                && !trimmed.starts_with("while"))
                || trimmed.starts_with("get ")
                || trimmed.starts_with("set ")
                || trimmed.starts_with("async ")
                || trimmed.starts_with("static ")
                || trimmed.starts_with("constructor")
        }
        "python" => trimmed.starts_with("def ") || trimmed.starts_with("async def "),
        "go" => trimmed.starts_with("func ") || trimmed.contains(" func("),
        _ => {
            // Generic: look for function-like patterns
            trimmed.starts_with("fn ")
                || trimmed.starts_with("func ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("function ")
                || trimmed.starts_with("pub fn ")
        }
    }
}

/// Generate a skeleton preview for container types (impl, class, trait).
/// Shows the opening line + method signatures.
fn skeleton_preview(text: &str, grammar: &str) -> (String, usize) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return (String::new(), 0);
    }

    let mut preview_lines = Vec::new();
    let mut method_count = 0;

    // Always include the first line (impl X for Y, class Foo, etc.)
    preview_lines.push(lines[0]);

    // Scan for method signatures
    for line in &lines[1..] {
        if is_signature_line(line, grammar) {
            method_count += 1;
            // Include the signature, but truncate long bodies on same line
            let sig = if let Some(brace_pos) = line.find('{') {
                let before_brace = &line[..brace_pos + 1];
                if line.trim().ends_with('}') && line.len() < 100 {
                    // Short one-liner, include it
                    *line
                } else {
                    before_brace
                }
            } else {
                *line
            };
            preview_lines.push(sig);
        }
    }

    let hidden = lines.len().saturating_sub(preview_lines.len());
    let mut result = preview_lines.join("\n");

    // Add closing brace for containers
    if method_count > 0 && !result.trim().ends_with('}') {
        let indent: String = lines[0].chars().take_while(|c| c.is_whitespace()).collect();
        result.push_str(&format!("\n{}}}", indent));
    }

    (result, hidden)
}

fn is_rust_structural(trimmed: &str) -> bool {
    trimmed.starts_with("let ")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("} else")
        || trimmed.starts_with("else ")
        || trimmed.starts_with("match ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("loop ")
        || trimmed.starts_with("return ")
        || trimmed.starts_with("return;")
        || trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("async fn ")
        || trimmed.contains("?;")
}

fn is_js_structural(trimmed: &str) -> bool {
    trimmed.starts_with("const ")
        || trimmed.starts_with("let ")
        || trimmed.starts_with("var ")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("} else")
        || trimmed.starts_with("else ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("switch ")
        || trimmed.starts_with("return ")
        || trimmed.starts_with("return;")
        || trimmed.starts_with("function ")
        || trimmed.starts_with("async ")
        || trimmed.starts_with("await ")
        || trimmed.starts_with("try ")
        || trimmed.starts_with("} catch")
}

fn is_python_structural(trimmed: &str) -> bool {
    trimmed.ends_with(':')
        || trimmed.starts_with("return ")
        || trimmed.starts_with("yield ")
        || trimmed.starts_with("raise ")
        || trimmed.starts_with("def ")
        || trimmed.starts_with("async def ")
}

fn is_go_structural(trimmed: &str) -> bool {
    trimmed.starts_with("if ")
        || trimmed.starts_with("} else")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("switch ")
        || trimmed.starts_with("select ")
        || trimmed.starts_with("return ")
        || trimmed.starts_with("func ")
        || trimmed.contains(":= ")
}

fn is_generic_structural(trimmed: &str) -> bool {
    trimmed.starts_with("let ")
        || trimmed.starts_with("const ")
        || trimmed.starts_with("var ")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("return ")
}

/// Check if a line represents structural code (control flow, bindings).
fn is_structural_line(line: &str, grammar: &str) -> bool {
    let trimmed = line.trim();

    // Skip empty lines, comments, and pure braces
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed == "{"
        || trimmed == "}"
    {
        return false;
    }

    match grammar {
        "rust" => is_rust_structural(trimmed),
        "javascript" | "typescript" | "tsx" => is_js_structural(trimmed),
        "python" => is_python_structural(trimmed),
        "go" => is_go_structural(trimmed),
        _ => is_generic_structural(trimmed),
    }
}

/// Generate a structural preview for functions.
/// Shows signature + control flow + bindings.
fn structural_preview(text: &str, grammar: &str) -> (String, usize) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return (String::new(), 0);
    }

    let mut preview_lines = Vec::new();
    const MAX_STRUCTURAL_LINES: usize = 20;

    // Always include the first line (function signature)
    preview_lines.push(lines[0]);

    // Collect structural lines
    for line in &lines[1..] {
        if is_structural_line(line, grammar) && preview_lines.len() < MAX_STRUCTURAL_LINES {
            preview_lines.push(*line);
        }
    }

    // Always include the closing brace if present
    if let Some(last) = lines.last()
        && last.trim() == "}"
        && !preview_lines.contains(last)
    {
        preview_lines.push(*last);
    }

    let hidden = lines.len().saturating_sub(preview_lines.len());
    let result = preview_lines.join("\n");

    (result, hidden)
}

/// Match result from either pattern type.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct MatchResult {
    pub file: PathBuf,
    pub grammar: String,
    pub kind: String,
    pub text: String,
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
    pub captures: HashMap<String, String>,
}

/// Run query against a single file using tree-sitter S-expression.
fn run_sexp_query(
    file: &Path,
    content: &str,
    query_str: &str,
    grammar: &tree_sitter::Language,
    grammar_name: &str,
) -> Result<Vec<MatchResult>, String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(grammar)
        .map_err(|e| format!("Failed to set language: {}", e))?;

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| "Failed to parse file".to_string())?;

    let query =
        tree_sitter::Query::new(grammar, query_str).map_err(|e| format!("Invalid query: {}", e))?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches_iter = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut results = Vec::new();
    while let Some(m) = matches_iter.next() {
        if !evaluate_predicates(&query, m, content.as_bytes()) {
            continue;
        }

        for cap in m.captures {
            let node = cap.node;
            let capture_name = query.capture_names()[cap.index as usize].to_string();
            let text = node.utf8_text(content.as_bytes()).unwrap_or("").to_string();

            let mut captures = HashMap::new();
            captures.insert(capture_name.clone(), text.clone());

            results.push(MatchResult {
                file: file.to_path_buf(),
                grammar: grammar_name.to_string(),
                kind: node.kind().to_string(),
                text,
                start_row: node.start_position().row + 1,
                start_col: node.start_position().column + 1,
                end_row: node.end_position().row + 1,
                end_col: node.end_position().column + 1,
                captures,
            });
        }
    }

    Ok(results)
}

/// Run query against a single file using ast-grep pattern.
fn run_astgrep_query(
    file: &Path,
    content: &str,
    pattern_str: &str,
    grammar: &tree_sitter::Language,
    grammar_name: &str,
) -> Result<Vec<MatchResult>, String> {
    use ast_grep_core::tree_sitter::LanguageExt;

    let lang = DynLang::new(grammar.clone());
    let grep = lang.ast_grep(content);
    let pattern = lang
        .pattern(pattern_str)
        .map_err(|e| format!("Pattern error: {:?}", e))?;

    let mut results = Vec::new();
    let root = grep.root();
    for node_match in root.find_all(&pattern) {
        let text = node_match.text().to_string();
        let start_pos = node_match.start_pos();
        let end_pos = node_match.end_pos();

        // For ast-grep, captures are in the MetaVarEnv, but extracting them
        // is complex. For now, just report the matched text.
        let captures = HashMap::new();

        results.push(MatchResult {
            file: file.to_path_buf(),
            grammar: grammar_name.to_string(),
            kind: node_match.kind().to_string(),
            text,
            start_row: start_pos.line() + 1,
            start_col: start_pos.column(&node_match) + 1,
            end_row: end_pos.line() + 1,
            end_col: end_pos.column(&node_match) + 1,
            captures,
        });
    }

    Ok(results)
}

/// Collect files to search based on path argument.
fn collect_files(path: Option<&Path>, filter: Option<&Filter>) -> Vec<PathBuf> {
    let root = path.unwrap_or(Path::new("."));

    if root.is_file() {
        return vec![root.to_path_buf()];
    }

    let mut files = Vec::new();
    collect_files_recursive(root, filter, &mut files);
    files
}

fn collect_files_recursive(dir: &Path, filter: Option<&Filter>, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip hidden and common non-source directories
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "vendor"
            {
                continue;
            }
            collect_files_recursive(&path, filter, files);
        } else if path.is_file() {
            // Only include files we have language support for
            let matches_filter = filter.map(|f| f.matches(&path)).unwrap_or(true);
            if support_for_path(&path).is_some() && matches_filter {
                files.push(path);
            }
        }
    }
}

/// Run a query and return results without printing (for service layer).
pub fn run_query_service(
    pattern: &str,
    path: Option<&std::path::Path>,
    _show_source: bool,
    _context_lines: usize,
    root: &std::path::Path,
    filter: Option<&crate::filter::Filter>,
) -> Result<Vec<MatchResult>, String> {
    let is_sexp = is_sexp_pattern(pattern);
    let loader = grammar_loader();

    // If path is provided use it, otherwise use root
    let search_path = path.unwrap_or(root);
    let files = collect_files(Some(search_path), filter);

    if files.is_empty() {
        return Ok(Vec::new());
    }

    let mut by_grammar: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in files {
        if let Some(lang) = support_for_path(&file) {
            by_grammar
                .entry(lang.grammar_name().to_string())
                .or_default()
                .push(file);
        }
    }

    let mut all_results = Vec::new();

    for (grammar_name, files) in by_grammar {
        let Some(grammar) = loader.get(&grammar_name) else {
            continue;
        };

        for file in files {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let results = if is_sexp {
                run_sexp_query(&file, &content, pattern, &grammar, &grammar_name)
            } else {
                run_astgrep_query(&file, &content, pattern, &grammar, &grammar_name)
            };

            match results {
                Ok(r) => all_results.extend(r),
                Err(e) => {
                    if e.contains("Invalid query") || e.contains("Pattern error") {
                        return Err(e);
                    }
                    // Skip per-file errors silently
                }
            }
        }
    }

    Ok(all_results)
}

/// Test a query against files.
///
/// Supports both tree-sitter S-expression queries and ast-grep patterns.
pub fn cmd_query(
    pattern: &str,
    path: Option<&Path>,
    filter: Option<&Filter>,
    show_source: bool,
    context_lines: usize,
    format: &OutputFormat,
) -> i32 {
    let is_sexp = is_sexp_pattern(pattern);
    let loader = grammar_loader();
    let files = collect_files(path, filter);

    if files.is_empty() {
        eprintln!("No files to search");
        return 1;
    }

    // Group files by grammar for efficient processing
    let mut by_grammar: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in files {
        if let Some(lang) = support_for_path(&file) {
            by_grammar
                .entry(lang.grammar_name().to_string())
                .or_default()
                .push(file);
        }
    }

    let mut all_results = Vec::new();
    let mut errors = Vec::new();

    for (grammar_name, files) in by_grammar {
        let Some(grammar) = loader.get(&grammar_name) else {
            errors.push(format!("Grammar not found: {}", grammar_name));
            continue;
        };

        for file in files {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("{}: {}", file.display(), e));
                    continue;
                }
            };

            let results = if is_sexp {
                run_sexp_query(&file, &content, pattern, &grammar, &grammar_name)
            } else {
                run_astgrep_query(&file, &content, pattern, &grammar, &grammar_name)
            };

            match results {
                Ok(r) => all_results.extend(r),
                Err(e) => {
                    // For pattern errors, fail immediately (user needs to fix pattern)
                    if e.contains("Invalid query") || e.contains("Pattern error") {
                        eprintln!("{}", e);
                        return 1;
                    }
                    errors.push(format!("{}: {}", file.display(), e));
                }
            }
        }
    }

    // Output results
    if format.is_json() {
        let results: Vec<_> = all_results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "file": r.file.display().to_string(),
                    "kind": r.kind,
                    "text": r.text,
                    "start": { "row": r.start_row, "column": r.start_col },
                    "end": { "row": r.end_row, "column": r.end_col },
                    "captures": r.captures,
                })
            })
            .collect();

        let json_value = serde_json::Value::Array(results.clone());

        match format {
            OutputFormat::Jq { filter, jsonl } => {
                match crate::output::apply_jq(&json_value, filter) {
                    Ok(lines) => {
                        crate::output::print_jq_lines(&lines, *jsonl);
                    }
                    Err(e) => {
                        eprintln!("jq error: {}", e);
                        return 1;
                    }
                }
            }
            OutputFormat::JsonLines => {
                // Emit each result on its own line
                for item in results {
                    println!("{}", serde_json::to_string(&item).unwrap_or_default());
                }
            }
            _ => {
                println!("{}", serde_json::to_string_pretty(&json_value).unwrap());
            }
        }
    } else {
        let use_colors = format.use_colors();

        // Header
        if use_colors {
            use nu_ansi_term::Color;
            println!(
                "{} matches:",
                Color::Green.bold().paint(all_results.len().to_string())
            );
        } else {
            println!("{} matches:", all_results.len());
        }
        println!();

        for r in &all_results {
            // File location
            let location = format!(
                "{}:{}:{}-{}:{}",
                r.file.display(),
                r.start_row,
                r.start_col,
                r.end_row,
                r.end_col
            );
            if use_colors {
                use nu_ansi_term::Color;
                println!("{}", Color::Cyan.paint(&location));
            } else {
                println!("{}", location);
            }

            // Preview with syntax highlighting
            // - Containers (impl, class, trait): show skeleton (method signatures)
            // - Functions: show structural view (control flow, bindings)
            // - Other nodes: show up to context_lines
            // - --show-source: show everything
            let lines: Vec<&str> = r.text.lines().collect();
            let total_lines = lines.len();
            let is_container = CONTAINER_KINDS.contains(&r.kind.as_str());
            let is_function = FUNCTION_KINDS.contains(&r.kind.as_str());

            let (preview_text, remaining, preview_kind) =
                if show_source || total_lines <= context_lines {
                    (r.text.clone(), 0, PreviewKind::Full)
                } else if is_container {
                    let (text, hidden) = skeleton_preview(&r.text, &r.grammar);
                    (text, hidden, PreviewKind::Skeleton)
                } else if is_function {
                    let (text, hidden) = structural_preview(&r.text, &r.grammar);
                    (text, hidden, PreviewKind::Structural)
                } else {
                    let preview = lines[..context_lines].join("\n");
                    (preview, total_lines - context_lines, PreviewKind::Truncated)
                };

            let highlighted = highlight_source(&preview_text, &r.grammar, use_colors);
            for line in highlighted.lines() {
                println!("  {}", line);
            }

            if remaining > 0 {
                let msg = match preview_kind {
                    PreviewKind::Skeleton => format!(
                        "... ({} lines, {} methods)",
                        total_lines,
                        preview_text.lines().count().saturating_sub(2)
                    ),
                    PreviewKind::Structural => {
                        format!("... ({} lines, showing structure)", total_lines)
                    }
                    PreviewKind::Full | PreviewKind::Truncated => {
                        format!("... ({} more lines)", remaining)
                    }
                };
                if use_colors {
                    use nu_ansi_term::Color;
                    println!("  {}", Color::DarkGray.paint(msg));
                } else {
                    println!("  {}", msg);
                }
            }

            // Captures (if any interesting ones)
            if !r.captures.is_empty() && r.captures.len() > 1 {
                for (name, value) in &r.captures {
                    let short_value = if value.len() > 40 {
                        format!("{}...", &value[..40])
                    } else {
                        value.clone()
                    };
                    if use_colors {
                        use nu_ansi_term::Color;
                        println!("    {}: {}", Color::Magenta.paint(name), short_value);
                    } else {
                        println!("    {}: {}", name, short_value);
                    }
                }
            }
            println!();
        }
    }

    // Report errors at the end
    if !errors.is_empty() && !format.is_json() {
        eprintln!();
        eprintln!("Errors ({}):", errors.len());
        for e in errors.iter().take(5) {
            eprintln!("  {}", e);
        }
        if errors.len() > 5 {
            eprintln!("  ... and {} more", errors.len() - 5);
        }
    }

    0
}
