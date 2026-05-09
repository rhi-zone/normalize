//! Snapshot tests for the Jinja2 CFG builder and Mermaid renderer.
//!
//! Tests are skipped gracefully if the Jinja2 grammar is not installed.
//! Jinja2 has no function-level CFG — tests process the whole template file.

use normalize_cfg::{FunctionId, builder::build, mermaid::render};
use normalize_languages::parsers::grammar_loader;

fn build_cfg_mermaid_jinja2(fixture_path: &str) -> String {
    let source = std::fs::read(fixture_path)
        .unwrap_or_else(|e| panic!("failed to read {fixture_path}: {e}"));

    let loader = grammar_loader();
    let ts_lang = match loader.get("jinja2") {
        Ok(l) => l,
        Err(_) => return "jinja2 grammar not installed — skipped".to_string(),
    };
    let cfg_query = match loader.get_cfg("jinja2") {
        Some(q) => q,
        None => return "no jinja2 cfg query — skipped".to_string(),
    };

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).expect("set language");
    let tree = parser.parse(&source, None).expect("parse");

    let function_id = FunctionId {
        file: fixture_path.to_string(),
        qualified_name: "<template>".to_string(),
        start_line: 1,
    };

    let cfg = build(&tree, &cfg_query, &source, function_id, 0..source.len()).expect("CFG build");

    render(&cfg)
}

#[test]
fn test_jinja2_branch() {
    let mermaid = build_cfg_mermaid_jinja2("tests/fixtures/jinja2/branch.jinja2");
    insta::assert_snapshot!(mermaid);
}

#[test]
fn test_jinja2_loop() {
    let mermaid = build_cfg_mermaid_jinja2("tests/fixtures/jinja2/loop_.jinja2");
    insta::assert_snapshot!(mermaid);
}
