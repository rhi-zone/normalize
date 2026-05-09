//! Snapshot tests for the Java CFG builder and Mermaid renderer.
//!
//! Tests are skipped gracefully if the Java grammar is not installed.
//!
//! The labeled_break test validates that Java labeled break/continue are
//! captured as @cfg.exit.break / @cfg.exit.continue (label resolution
//! to the correct enclosing loop is tracked in TODO.md).

use normalize_cfg::{FunctionId, builder::build, mermaid::render};
use normalize_languages::parsers::grammar_loader;
use streaming_iterator::StreamingIterator;

fn build_cfg_mermaid_java(fixture_path: &str, function_name: Option<&str>) -> String {
    let source = std::fs::read(fixture_path)
        .unwrap_or_else(|e| panic!("failed to read {fixture_path}: {e}"));

    let loader = grammar_loader();
    let ts_lang = match loader.get("java") {
        Ok(l) => l,
        Err(_) => return "java grammar not installed — skipped".to_string(),
    };
    let cfg_query = match loader.get_cfg("java") {
        Some(q) => q,
        None => return "no java cfg query — skipped".to_string(),
    };

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).expect("set language");
    let tree = parser.parse(&source, None).expect("parse");

    let tags_query_src = match loader.get_tags("java") {
        Some(q) => q,
        None => return "no java tags query — skipped".to_string(),
    };
    let tags_query = tree_sitter::Query::new(&ts_lang, &tags_query_src).expect("compile tags");
    let capture_names = tags_query.capture_names().to_vec();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches_iter = cursor.matches(&tags_query, tree.root_node(), source.as_slice());

    let mut func_name = String::new();
    let mut body_start = 0usize;
    let mut body_end = source.len();
    let mut start_line = 1u32;

    while let Some(mat) = matches_iter.next() {
        for cap in mat.captures {
            let name = capture_names[cap.index as usize];
            if name.starts_with("name.definition.function")
                || name.starts_with("name.definition.method")
            {
                let candidate = cap
                    .node
                    .utf8_text(&source)
                    .unwrap_or("<unknown>")
                    .to_string();
                if function_name.is_some_and(|f| candidate != f) {
                    continue;
                }
                let def_node = cap.node.parent().unwrap_or(cap.node);
                func_name = candidate;
                body_start = def_node.start_byte();
                body_end = def_node.end_byte();
                start_line = def_node.start_position().row as u32 + 1;
                break;
            }
        }
        if !func_name.is_empty() {
            break;
        }
    }
    drop(matches_iter);

    if func_name.is_empty() {
        func_name = function_name.unwrap_or("<file>").to_string();
    }

    let function_id = FunctionId {
        file: fixture_path.to_string(),
        qualified_name: func_name,
        start_line,
    };

    let cfg = build(
        &tree,
        &cfg_query,
        &source,
        function_id,
        body_start..body_end,
    )
    .expect("CFG build");

    render(&cfg)
}

#[test]
fn test_java_linear() {
    let mermaid = build_cfg_mermaid_java("tests/fixtures/java/linear.java", Some("linear"));
    insta::assert_snapshot!(mermaid);
}

#[test]
fn test_java_branch() {
    let mermaid = build_cfg_mermaid_java("tests/fixtures/java/branch.java", Some("branch"));
    insta::assert_snapshot!(mermaid);
}

#[test]
fn test_java_loop() {
    let mermaid = build_cfg_mermaid_java("tests/fixtures/java/loop_.java", Some("loop_"));
    insta::assert_snapshot!(mermaid);
}

#[test]
fn test_java_labeled_break() {
    let mermaid = build_cfg_mermaid_java(
        "tests/fixtures/java/labeled_break.java",
        Some("labeled_break"),
    );
    insta::assert_snapshot!(mermaid);
}

#[test]
fn test_java_try_catch() {
    let mermaid = build_cfg_mermaid_java("tests/fixtures/java/try_catch.java", Some("try_catch"));
    insta::assert_snapshot!(mermaid);
}
