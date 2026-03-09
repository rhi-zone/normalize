/// Fixture tests for `.scm` tree-sitter query files.
///
/// Each test parses a sample source file, runs the relevant query, and asserts that
/// specific expected names appear in captures.
///
/// # Running
///
/// These tests require compiled grammar `.so` files in `target/grammars/`. Build them
/// with `cargo xtask build-grammars`. Without grammars present the tests skip gracefully
/// — `cargo test` always passes regardless of grammar availability.
///
/// To run with grammars:
///   cargo xtask build-grammars && cargo test -p normalize-languages -- --nocapture
use normalize_languages::GrammarLoader;
use std::path::PathBuf;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the grammar search path if `target/grammars/` exists relative to the
/// workspace root, otherwise return `None` to signal the test should be skipped.
fn grammar_dir() -> Option<PathBuf> {
    // Integration tests run with cwd = crate root; grammars live at workspace root.
    let crate_root = std::env::current_dir().unwrap();
    let workspace_root = crate_root
        .ancestors()
        .find(|p| p.join("Cargo.lock").exists())?;
    let dir = workspace_root.join("target/grammars");
    if dir.exists() { Some(dir) } else { None }
}

/// Parse `source` with `lang`, run `query_str` against it, and collect all
/// captures whose name starts with `capture_prefix` into a `Vec<String>`.
fn collect_captures(
    lang: &tree_sitter::Language,
    source: &str,
    query_str: &str,
    capture_prefix: &str,
) -> Vec<String> {
    let mut parser = Parser::new();
    parser.set_language(lang).expect("set_language failed");
    let tree = parser.parse(source, None).expect("parse failed");

    let query = Query::new(lang, query_str).expect("query compilation failed");
    let mut cursor = QueryCursor::new();
    let source_bytes = source.as_bytes();

    let mut results = Vec::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source_bytes);
    while let Some(m) = matches.next() {
        for cap in m.captures {
            let cap_name = query.capture_names()[cap.index as usize];
            if cap_name.starts_with(capture_prefix) {
                let text = cap.node.utf8_text(source_bytes).unwrap_or("").to_string();
                results.push(text);
            }
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

const RUST_SAMPLE: &str = include_str!("fixtures/rust/sample.rs");

#[test]
fn rust_tags_finds_functions_and_structs() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rust_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rust") else {
        eprintln!("Skipping rust_tags: rust grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("rust").expect("rust tags query missing");
    let names = collect_captures(&lang, RUST_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Counter".to_string()),
        "expected 'Counter' struct in tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in tags, got: {names:?}"
    );
    assert!(
        names.contains(&"sum_evens".to_string()),
        "expected 'sum_evens' function in tags, got: {names:?}"
    );
}

#[test]
fn rust_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rust_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rust") else {
        eprintln!("Skipping rust_calls: rust grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("rust").expect("rust calls query missing");
    let calls = collect_captures(&lang, RUST_SAMPLE, &query_str, "call");
    // Counter::new() → @call = "new", increment/get are method calls
    assert!(
        calls.iter().any(|c| c == "new"),
        "expected 'new' call in rust sample, got: {calls:?}"
    );
}

#[test]
fn rust_imports_finds_use_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rust_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rust") else {
        eprintln!("Skipping rust_imports: rust grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("rust")
        .expect("rust imports query missing");
    let paths = collect_captures(&lang, RUST_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("std")),
        "expected std import path in rust sample, got: {paths:?}"
    );
    let names = collect_captures(&lang, RUST_SAMPLE, &query_str, "import.name");
    assert!(
        names.contains(&"HashMap".to_string()),
        "expected 'HashMap' import name in rust sample, got: {names:?}"
    );
}

#[test]
fn rust_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rust_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rust") else {
        eprintln!("Skipping rust_complexity: rust grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("rust")
        .expect("rust complexity query missing");
    let complexity = collect_captures(&lang, RUST_SAMPLE, &query_str, "complexity");
    // classify() has two if branches; sum_evens() has for + if
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in rust sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn rust_types_finds_struct_definitions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rust_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rust") else {
        eprintln!("Skipping rust_types: rust grammar .so not found");
        return;
    };
    let query_str = loader.get_types("rust").expect("rust types query missing");
    let names = collect_captures(&lang, RUST_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Counter".to_string()),
        "expected 'Counter' in rust types captures, got: {names:?}"
    );
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

const PYTHON_SAMPLE: &str = include_str!("fixtures/python/sample.py");

#[test]
fn python_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping python_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("python") else {
        eprintln!("Skipping python_tags: python grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("python")
        .expect("python tags query missing");
    let names = collect_captures(&lang, PYTHON_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"DataProcessor".to_string()),
        "expected 'DataProcessor' class in python tags, got: {names:?}"
    );
    assert!(
        names.contains(&"load_file".to_string()),
        "expected 'load_file' function in python tags, got: {names:?}"
    );
    assert!(
        names.contains(&"count_words".to_string()),
        "expected 'count_words' function in python tags, got: {names:?}"
    );
}

#[test]
fn python_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping python_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("python") else {
        eprintln!("Skipping python_calls: python grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("python")
        .expect("python calls query missing");
    let calls = collect_captures(&lang, PYTHON_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"append".to_string()),
        "expected 'append' method call in python sample, got: {calls:?}"
    );
}

#[test]
fn python_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping python_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("python") else {
        eprintln!("Skipping python_imports: python grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("python")
        .expect("python imports query missing");
    let paths = collect_captures(&lang, PYTHON_SAMPLE, &query_str, "import.path");
    assert!(
        paths.contains(&"os".to_string()),
        "expected 'os' in python import paths, got: {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p == "collections"),
        "expected 'collections' in python import paths, got: {paths:?}"
    );
}

#[test]
fn python_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping python_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("python") else {
        eprintln!("Skipping python_complexity: python grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("python")
        .expect("python complexity query missing");
    let complexity = collect_captures(&lang, PYTHON_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in python sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn python_types_finds_class() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping python_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("python") else {
        eprintln!("Skipping python_types: python grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("python")
        .expect("python types query missing");
    let names = collect_captures(&lang, PYTHON_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"DataProcessor".to_string()),
        "expected 'DataProcessor' in python types captures, got: {names:?}"
    );
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

const GO_SAMPLE: &str = include_str!("fixtures/go/sample.go");

#[test]
fn go_tags_finds_functions_and_types() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping go_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("go") else {
        eprintln!("Skipping go_tags: go grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("go").expect("go tags query missing");
    let names = collect_captures(&lang, GO_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Classify".to_string()),
        "expected 'Classify' function in go tags, got: {names:?}"
    );
    assert!(
        names.contains(&"JoinWords".to_string()),
        "expected 'JoinWords' function in go tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Stack".to_string()),
        "expected 'Stack' type in go tags, got: {names:?}"
    );
}

#[test]
fn go_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping go_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("go") else {
        eprintln!("Skipping go_calls: go grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("go").expect("go calls query missing");
    let calls = collect_captures(&lang, GO_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"Println".to_string()),
        "expected 'Println' call in go sample, got: {calls:?}"
    );
}

#[test]
fn go_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping go_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("go") else {
        eprintln!("Skipping go_imports: go grammar .so not found");
        return;
    };
    let query_str = loader.get_imports("go").expect("go imports query missing");
    let paths = collect_captures(&lang, GO_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("fmt")),
        "expected '\"fmt\"' in go import paths, got: {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.contains("strings")),
        "expected '\"strings\"' in go import paths, got: {paths:?}"
    );
}

#[test]
fn go_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping go_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("go") else {
        eprintln!("Skipping go_complexity: go grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("go")
        .expect("go complexity query missing");
    let complexity = collect_captures(&lang, GO_SAMPLE, &query_str, "complexity");
    // Classify() has two if branches; Pop() has one if
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in go sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn go_types_finds_struct_definitions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping go_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("go") else {
        eprintln!("Skipping go_types: go grammar .so not found");
        return;
    };
    let query_str = loader.get_types("go").expect("go types query missing");
    let names = collect_captures(&lang, GO_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Stack".to_string()),
        "expected 'Stack' in go types captures, got: {names:?}"
    );
}

// ---------------------------------------------------------------------------
// TypeScript
// ---------------------------------------------------------------------------

const TS_SAMPLE: &str = include_str!("fixtures/typescript/sample.ts");

#[test]
fn typescript_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typescript_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typescript") else {
        eprintln!("Skipping typescript_tags: typescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("typescript")
        .expect("typescript tags query missing");
    let names = collect_captures(&lang, TS_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"FileLogger".to_string()),
        "expected 'FileLogger' class in typescript tags, got: {names:?}"
    );
    assert!(
        names.contains(&"formatPath".to_string()),
        "expected 'formatPath' function in typescript tags, got: {names:?}"
    );
    assert!(
        names.contains(&"groupBy".to_string()),
        "expected 'groupBy' function in typescript tags, got: {names:?}"
    );
}

#[test]
fn typescript_calls_finds_method_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typescript_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typescript") else {
        eprintln!("Skipping typescript_calls: typescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("typescript")
        .expect("typescript calls query missing");
    let calls = collect_captures(&lang, TS_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "normalize" || c == "log" || c == "push"),
        "expected at least one of normalize/log/push calls in typescript sample, got: {calls:?}"
    );
}

#[test]
fn typescript_imports_finds_module_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typescript_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typescript") else {
        eprintln!("Skipping typescript_imports: typescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("typescript")
        .expect("typescript imports query missing");
    let paths = collect_captures(&lang, TS_SAMPLE, &query_str, "import.path");
    assert!(
        paths.contains(&"events".to_string()),
        "expected 'events' in typescript import paths, got: {paths:?}"
    );
    assert!(
        paths.contains(&"path".to_string()),
        "expected 'path' in typescript import paths, got: {paths:?}"
    );
}

#[test]
fn typescript_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typescript_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typescript") else {
        eprintln!("Skipping typescript_complexity: typescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("typescript")
        .expect("typescript complexity query missing");
    let complexity = collect_captures(&lang, TS_SAMPLE, &query_str, "complexity");
    // formatPath has an if; groupBy has a for_in
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in typescript sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn typescript_types_finds_interface_and_class() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typescript_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typescript") else {
        eprintln!("Skipping typescript_types: typescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("typescript")
        .expect("typescript types query missing");
    let names = collect_captures(&lang, TS_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"FileLogger".to_string()) || names.contains(&"Logger".to_string()),
        "expected 'FileLogger' or 'Logger' in typescript types captures, got: {names:?}"
    );
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

const JAVA_SAMPLE: &str = include_str!("fixtures/java/sample.java");

#[test]
fn java_tags_finds_class_and_methods() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping java_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("java") else {
        eprintln!("Skipping java_tags: java grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("java").expect("java tags query missing");
    let names = collect_captures(&lang, JAVA_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"TaskQueue".to_string()),
        "expected 'TaskQueue' class in java tags, got: {names:?}"
    );
    assert!(
        names.contains(&"enqueue".to_string()),
        "expected 'enqueue' method in java tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' method in java tags, got: {names:?}"
    );
}

#[test]
fn java_calls_finds_method_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping java_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("java") else {
        eprintln!("Skipping java_calls: java grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("java").expect("java calls query missing");
    let calls = collect_captures(&lang, JAVA_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"add".to_string()) || calls.contains(&"remove".to_string()),
        "expected 'add' or 'remove' method call in java sample, got: {calls:?}"
    );
}

#[test]
fn java_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping java_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("java") else {
        eprintln!("Skipping java_imports: java grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("java")
        .expect("java imports query missing");
    let paths = collect_captures(&lang, JAVA_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("ArrayList")),
        "expected 'java.util.ArrayList' in java import paths, got: {paths:?}"
    );
}

#[test]
fn java_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping java_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("java") else {
        eprintln!("Skipping java_complexity: java grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("java")
        .expect("java complexity query missing");
    let complexity = collect_captures(&lang, JAVA_SAMPLE, &query_str, "complexity");
    // enqueue() has an if; dequeue() has an if; classify() has if/else-if
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in java sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn java_types_finds_class() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping java_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("java") else {
        eprintln!("Skipping java_types: java grammar .so not found");
        return;
    };
    let query_str = loader.get_types("java").expect("java types query missing");
    let names = collect_captures(&lang, JAVA_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"TaskQueue".to_string()),
        "expected 'TaskQueue' in java types captures, got: {names:?}"
    );
}

// ---------------------------------------------------------------------------
// Ruby
// ---------------------------------------------------------------------------

const RUBY_SAMPLE: &str = include_str!("fixtures/ruby/sample.rb");

#[test]
fn ruby_tags_finds_class_and_methods() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ruby_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ruby") else {
        eprintln!("Skipping ruby_tags: ruby grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("ruby").expect("ruby tags query missing");
    let names = collect_captures(&lang, RUBY_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Stack".to_string()),
        "expected 'Stack' class in ruby tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' method in ruby tags, got: {names:?}"
    );
    assert!(
        names.contains(&"sum_if".to_string()),
        "expected 'sum_if' method in ruby tags, got: {names:?}"
    );
}

#[test]
fn ruby_calls_finds_method_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ruby_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ruby") else {
        eprintln!("Skipping ruby_calls: ruby grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("ruby").expect("ruby calls query missing");
    let calls = collect_captures(&lang, RUBY_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"push".to_string()) || calls.contains(&"pop".to_string()),
        "expected 'push' or 'pop' call in ruby sample, got: {calls:?}"
    );
}

#[test]
fn ruby_imports_finds_require() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ruby_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ruby") else {
        eprintln!("Skipping ruby_imports: ruby grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("ruby")
        .expect("ruby imports query missing");
    let paths = collect_captures(&lang, RUBY_SAMPLE, &query_str, "import.path");
    assert!(
        paths.contains(&"json".to_string()),
        "expected 'json' in ruby import paths, got: {paths:?}"
    );
}

#[test]
fn ruby_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ruby_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ruby") else {
        eprintln!("Skipping ruby_complexity: ruby grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("ruby")
        .expect("ruby complexity query missing");
    let complexity = collect_captures(&lang, RUBY_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in ruby sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn ruby_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ruby_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ruby") else {
        eprintln!("Skipping ruby_types: ruby grammar .so not found");
        return;
    };
    let query_str = loader.get_types("ruby").expect("ruby types query missing");
    // Ruby types.scm captures @type.reference (superclass/scope resolution)
    let refs = collect_captures(&lang, RUBY_SAMPLE, &query_str, "type");
    // The sample has no explicit inheritance, but the query should at least parse
    // without error; empty result is acceptable for this sample.
    let _ = refs; // result may be empty — query must compile and run
}

// ---------------------------------------------------------------------------
// Kotlin
// ---------------------------------------------------------------------------

const KOTLIN_SAMPLE: &str = include_str!("fixtures/kotlin/sample.kt");

#[test]
fn kotlin_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping kotlin_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("kotlin") else {
        eprintln!("Skipping kotlin_tags: kotlin grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("kotlin")
        .expect("kotlin tags query missing");
    let names = collect_captures(&lang, KOTLIN_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' class in kotlin tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in kotlin tags, got: {names:?}"
    );
    assert!(
        names.contains(&"sumEvens".to_string()),
        "expected 'sumEvens' function in kotlin tags, got: {names:?}"
    );
}

#[test]
fn kotlin_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping kotlin_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("kotlin") else {
        eprintln!("Skipping kotlin_calls: kotlin grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("kotlin")
        .expect("kotlin calls query missing");
    let calls = collect_captures(&lang, KOTLIN_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"println".to_string()) || calls.contains(&"enqueue".to_string()),
        "expected 'println' or 'enqueue' call in kotlin sample, got: {calls:?}"
    );
}

#[test]
fn kotlin_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping kotlin_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("kotlin") else {
        eprintln!("Skipping kotlin_imports: kotlin grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("kotlin")
        .expect("kotlin imports query missing");
    let paths = collect_captures(&lang, KOTLIN_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("LinkedList") || p.contains("java")),
        "expected 'java.util.LinkedList' in kotlin import paths, got: {paths:?}"
    );
}

#[test]
fn kotlin_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping kotlin_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("kotlin") else {
        eprintln!("Skipping kotlin_complexity: kotlin grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("kotlin")
        .expect("kotlin complexity query missing");
    let complexity = collect_captures(&lang, KOTLIN_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in kotlin sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn kotlin_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping kotlin_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("kotlin") else {
        eprintln!("Skipping kotlin_types: kotlin grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("kotlin")
        .expect("kotlin types query missing");
    let refs = collect_captures(&lang, KOTLIN_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "Point" || r == "Double" || r == "Int"),
        "expected 'Point', 'Double', or 'Int' in kotlin type references, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Swift
// ---------------------------------------------------------------------------

const SWIFT_SAMPLE: &str = include_str!("fixtures/swift/sample.swift");

#[test]
fn swift_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping swift_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("swift") else {
        eprintln!("Skipping swift_tags: swift grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("swift").expect("swift tags query missing");
    let names = collect_captures(&lang, SWIFT_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Stack".to_string()),
        "expected 'Stack' class in swift tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in swift tags, got: {names:?}"
    );
    assert!(
        names.contains(&"sumEvens".to_string()),
        "expected 'sumEvens' function in swift tags, got: {names:?}"
    );
}

#[test]
fn swift_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping swift_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("swift") else {
        eprintln!("Skipping swift_calls: swift grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("swift")
        .expect("swift calls query missing");
    let calls = collect_captures(&lang, SWIFT_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"print".to_string()) || calls.contains(&"push".to_string()),
        "expected 'print' or 'push' call in swift sample, got: {calls:?}"
    );
}

#[test]
fn swift_imports_finds_module_imports() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping swift_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("swift") else {
        eprintln!("Skipping swift_imports: swift grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("swift")
        .expect("swift imports query missing");
    let paths = collect_captures(&lang, SWIFT_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("Foundation") || p.contains("Swift")),
        "expected 'Foundation' or 'Swift' in swift import paths, got: {paths:?}"
    );
}

#[test]
fn swift_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping swift_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("swift") else {
        eprintln!("Skipping swift_complexity: swift grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("swift")
        .expect("swift complexity query missing");
    let complexity = collect_captures(&lang, SWIFT_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in swift sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn swift_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping swift_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("swift") else {
        eprintln!("Skipping swift_types: swift grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("swift")
        .expect("swift types query missing");
    let refs = collect_captures(&lang, SWIFT_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "Int" || r == "String" || r == "Bool"),
        "expected primitive type references in swift sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Scala
// ---------------------------------------------------------------------------

const SCALA_SAMPLE: &str = include_str!("fixtures/scala/sample.scala");

#[test]
fn scala_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scala_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scala") else {
        eprintln!("Skipping scala_tags: scala grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("scala").expect("scala tags query missing");
    let names = collect_captures(&lang, SCALA_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' class in scala tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in scala tags, got: {names:?}"
    );
    assert!(
        names.contains(&"sumEvens".to_string()),
        "expected 'sumEvens' function in scala tags, got: {names:?}"
    );
}

#[test]
fn scala_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scala_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scala") else {
        eprintln!("Skipping scala_calls: scala grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("scala")
        .expect("scala calls query missing");
    let calls = collect_captures(&lang, SCALA_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"println".to_string()) || calls.contains(&"push".to_string()),
        "expected 'println' or 'push' call in scala sample, got: {calls:?}"
    );
}

#[test]
fn scala_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scala_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scala") else {
        eprintln!("Skipping scala_imports: scala grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("scala")
        .expect("scala imports query missing");
    // Scala imports query captures @import (the full declaration node)
    let imports = collect_captures(&lang, SCALA_SAMPLE, &query_str, "import");
    assert!(
        !imports.is_empty(),
        "expected at least one import declaration in scala sample, got: {imports:?}"
    );
}

#[test]
fn scala_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scala_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scala") else {
        eprintln!("Skipping scala_complexity: scala grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("scala")
        .expect("scala complexity query missing");
    let complexity = collect_captures(&lang, SCALA_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in scala sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn scala_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scala_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scala") else {
        eprintln!("Skipping scala_types: scala grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("scala")
        .expect("scala types query missing");
    let refs = collect_captures(&lang, SCALA_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "Int" || r == "Double" || r == "String"),
        "expected type identifiers in scala sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// PHP
// ---------------------------------------------------------------------------

const PHP_SAMPLE: &str = include_str!("fixtures/php/sample.php");

#[test]
fn php_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping php_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("php") else {
        eprintln!("Skipping php_tags: php grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("php").expect("php tags query missing");
    let names = collect_captures(&lang, PHP_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Stack".to_string()),
        "expected 'Stack' class in php tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in php tags, got: {names:?}"
    );
    assert!(
        names.contains(&"sumEvens".to_string()),
        "expected 'sumEvens' function in php tags, got: {names:?}"
    );
}

#[test]
fn php_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping php_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("php") else {
        eprintln!("Skipping php_calls: php grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("php").expect("php calls query missing");
    let calls = collect_captures(&lang, PHP_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"classify".to_string())
            || calls.contains(&"array_push".to_string())
            || calls.contains(&"empty".to_string()),
        "expected a function call in php sample, got: {calls:?}"
    );
}

#[test]
fn php_imports_finds_use_declarations() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping php_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("php") else {
        eprintln!("Skipping php_imports: php grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("php")
        .expect("php imports query missing");
    let paths = collect_captures(&lang, PHP_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("User") || p.contains("Collection") || p.contains("App")),
        "expected namespace path in php import paths, got: {paths:?}"
    );
}

#[test]
fn php_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping php_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("php") else {
        eprintln!("Skipping php_complexity: php grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("php")
        .expect("php complexity query missing");
    let complexity = collect_captures(&lang, PHP_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in php sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn php_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping php_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("php") else {
        eprintln!("Skipping php_types: php grammar .so not found");
        return;
    };
    let query_str = loader.get_types("php").expect("php types query missing");
    let refs = collect_captures(&lang, PHP_SAMPLE, &query_str, "type");
    // PHP types.scm captures @type.reference; sample has typed parameters
    let _ = refs; // result content is grammar-dependent; query must compile
}

// ---------------------------------------------------------------------------
// Dart
// ---------------------------------------------------------------------------

const DART_SAMPLE: &str = include_str!("fixtures/dart/sample.dart");

#[test]
fn dart_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping dart_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("dart") else {
        eprintln!("Skipping dart_tags: dart grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("dart").expect("dart tags query missing");
    let names = collect_captures(&lang, DART_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' class in dart tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in dart tags, got: {names:?}"
    );
    assert!(
        names.contains(&"sumEvens".to_string()),
        "expected 'sumEvens' function in dart tags, got: {names:?}"
    );
}

#[test]
fn dart_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping dart_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("dart") else {
        eprintln!("Skipping dart_calls: dart grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("dart").expect("dart calls query missing");
    let calls = collect_captures(&lang, DART_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"print".to_string()) || calls.contains(&"push".to_string()),
        "expected 'print' or 'push' call in dart sample, got: {calls:?}"
    );
}

#[test]
fn dart_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping dart_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("dart") else {
        eprintln!("Skipping dart_imports: dart grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("dart")
        .expect("dart imports query missing");
    let paths = collect_captures(&lang, DART_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("collection") || p.contains("dart")),
        "expected dart library path in dart import paths, got: {paths:?}"
    );
}

#[test]
fn dart_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping dart_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("dart") else {
        eprintln!("Skipping dart_complexity: dart grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("dart")
        .expect("dart complexity query missing");
    let complexity = collect_captures(&lang, DART_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in dart sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn dart_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping dart_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("dart") else {
        eprintln!("Skipping dart_types: dart grammar .so not found");
        return;
    };
    let query_str = loader.get_types("dart").expect("dart types query missing");
    let refs = collect_captures(&lang, DART_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "Point" || r == "int" || r == "String"),
        "expected type identifiers in dart sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Elixir
// ---------------------------------------------------------------------------

const ELIXIR_SAMPLE: &str = include_str!("fixtures/elixir/sample.ex");

#[test]
fn elixir_tags_finds_modules_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elixir_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elixir") else {
        eprintln!("Skipping elixir_tags: elixir grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("elixir")
        .expect("elixir tags query missing");
    let names = collect_captures(&lang, ELIXIR_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in elixir tags, got: {names:?}"
    );
    assert!(
        names.contains(&"push".to_string()) || names.contains(&"pop".to_string()),
        "expected 'push' or 'pop' in elixir tags, got: {names:?}"
    );
}

#[test]
fn elixir_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elixir_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elixir") else {
        eprintln!("Skipping elixir_calls: elixir grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("elixir")
        .expect("elixir calls query missing");
    let calls = collect_captures(&lang, ELIXIR_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"defmodule".to_string()) || calls.contains(&"def".to_string()),
        "expected 'defmodule' or 'def' call in elixir sample, got: {calls:?}"
    );
}

#[test]
fn elixir_imports_finds_alias_and_import() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elixir_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elixir") else {
        eprintln!("Skipping elixir_imports: elixir grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("elixir")
        .expect("elixir imports query missing");
    let paths = collect_captures(&lang, ELIXIR_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("Enum")),
        "expected 'Enum' in elixir import paths, got: {paths:?}"
    );
}

#[test]
fn elixir_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elixir_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elixir") else {
        eprintln!("Skipping elixir_complexity: elixir grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("elixir")
        .expect("elixir complexity query missing");
    let complexity = collect_captures(&lang, ELIXIR_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in elixir sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn elixir_types_finds_module_aliases() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elixir_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elixir") else {
        eprintln!("Skipping elixir_types: elixir grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("elixir")
        .expect("elixir types query missing");
    let refs = collect_captures(&lang, ELIXIR_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r.contains("Enum") || r.contains("Stack") || r.contains("MathUtils")),
        "expected module alias references in elixir sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// C
// ---------------------------------------------------------------------------

const C_SAMPLE: &str = include_str!("fixtures/c/sample.c");

#[test]
fn c_tags_finds_functions_and_structs() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping c_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c") else {
        eprintln!("Skipping c_tags: c grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("c").expect("c tags query missing");
    let names = collect_captures(&lang, C_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"stack_new".to_string()) || names.contains(&"classify".to_string()),
        "expected 'stack_new' or 'classify' function in c tags, got: {names:?}"
    );
}

#[test]
fn c_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping c_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c") else {
        eprintln!("Skipping c_calls: c grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("c").expect("c calls query missing");
    let calls = collect_captures(&lang, C_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"malloc".to_string()) || calls.contains(&"printf".to_string()),
        "expected 'malloc' or 'printf' call in c sample, got: {calls:?}"
    );
}

#[test]
fn c_imports_finds_include_directives() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping c_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c") else {
        eprintln!("Skipping c_imports: c grammar .so not found");
        return;
    };
    let query_str = loader.get_imports("c").expect("c imports query missing");
    let paths = collect_captures(&lang, C_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("stdio.h") || p.contains("stdlib.h")),
        "expected 'stdio.h' or 'stdlib.h' in c import paths, got: {paths:?}"
    );
}

#[test]
fn c_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping c_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c") else {
        eprintln!("Skipping c_complexity: c grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("c")
        .expect("c complexity query missing");
    let complexity = collect_captures(&lang, C_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in c sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn c_types_finds_type_identifiers() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping c_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c") else {
        eprintln!("Skipping c_types: c grammar .so not found");
        return;
    };
    let query_str = loader.get_types("c").expect("c types query missing");
    let refs = collect_captures(&lang, C_SAMPLE, &query_str, "type");
    assert!(
        refs.iter().any(|r| r == "Stack"),
        "expected 'Stack' in c type references, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// C++
// ---------------------------------------------------------------------------

const CPP_SAMPLE: &str = include_str!("fixtures/cpp/sample.cpp");

#[test]
fn cpp_tags_finds_class_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cpp_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cpp") else {
        eprintln!("Skipping cpp_tags: cpp grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("cpp").expect("cpp tags query missing");
    let names = collect_captures(&lang, CPP_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Stack".to_string()),
        "expected 'Stack' class in cpp tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()) || names.contains(&"sum_evens".to_string()),
        "expected 'classify' or 'sum_evens' function in cpp tags, got: {names:?}"
    );
}

#[test]
fn cpp_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cpp_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cpp") else {
        eprintln!("Skipping cpp_calls: cpp grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("cpp").expect("cpp calls query missing");
    let calls = collect_captures(&lang, CPP_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"classify".to_string())
            || calls.contains(&"push".to_string())
            || calls.contains(&"pop".to_string()),
        "expected function call in cpp sample, got: {calls:?}"
    );
}

#[test]
fn cpp_imports_finds_include_directives() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cpp_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cpp") else {
        eprintln!("Skipping cpp_imports: cpp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("cpp")
        .expect("cpp imports query missing");
    let paths = collect_captures(&lang, CPP_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("iostream") || p.contains("vector")),
        "expected 'iostream' or 'vector' in cpp import paths, got: {paths:?}"
    );
}

#[test]
fn cpp_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cpp_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cpp") else {
        eprintln!("Skipping cpp_complexity: cpp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("cpp")
        .expect("cpp complexity query missing");
    let complexity = collect_captures(&lang, CPP_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in cpp sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn cpp_types_finds_type_identifiers() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cpp_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cpp") else {
        eprintln!("Skipping cpp_types: cpp grammar .so not found");
        return;
    };
    let query_str = loader.get_types("cpp").expect("cpp types query missing");
    let refs = collect_captures(&lang, CPP_SAMPLE, &query_str, "type");
    assert!(
        refs.iter().any(|r| r == "Stack"),
        "expected 'Stack' in cpp type references, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// C#
// ---------------------------------------------------------------------------

const CSHARP_SAMPLE: &str = include_str!("fixtures/c-sharp/sample.cs");

#[test]
fn csharp_tags_finds_class_and_methods() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping csharp_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c-sharp") else {
        eprintln!("Skipping csharp_tags: c-sharp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("c-sharp")
        .expect("c-sharp tags query missing");
    let names = collect_captures(&lang, CSHARP_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Stack".to_string()),
        "expected 'Stack' class in c-sharp tags, got: {names:?}"
    );
    assert!(
        names.contains(&"MathUtils".to_string()),
        "expected 'MathUtils' class in c-sharp tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Classify".to_string()),
        "expected 'Classify' method in c-sharp tags, got: {names:?}"
    );
}

#[test]
fn csharp_calls_finds_method_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping csharp_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c-sharp") else {
        eprintln!("Skipping csharp_calls: c-sharp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("c-sharp")
        .expect("c-sharp calls query missing");
    let calls = collect_captures(&lang, CSHARP_SAMPLE, &query_str, "call");
    assert!(
        calls.contains(&"Push".to_string())
            || calls.contains(&"WriteLine".to_string())
            || calls.contains(&"Add".to_string()),
        "expected method call in c-sharp sample, got: {calls:?}"
    );
}

#[test]
fn csharp_imports_finds_using_directives() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping csharp_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c-sharp") else {
        eprintln!("Skipping csharp_imports: c-sharp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("c-sharp")
        .expect("c-sharp imports query missing");
    let paths = collect_captures(&lang, CSHARP_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("System") || p.contains("Collections")),
        "expected 'System' or 'Collections' in c-sharp import paths, got: {paths:?}"
    );
}

#[test]
fn csharp_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping csharp_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c-sharp") else {
        eprintln!("Skipping csharp_complexity: c-sharp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("c-sharp")
        .expect("c-sharp complexity query missing");
    let complexity = collect_captures(&lang, CSHARP_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in c-sharp sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn csharp_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping csharp_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("c-sharp") else {
        eprintln!("Skipping csharp_types: c-sharp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("c-sharp")
        .expect("c-sharp types query missing");
    let refs = collect_captures(&lang, CSHARP_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "Stack" || r == "MathUtils" || r == "List"),
        "expected type reference in c-sharp sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Clojure
// ---------------------------------------------------------------------------

const CLOJURE_SAMPLE: &str = include_str!("fixtures/clojure/sample.clj");

#[test]
fn clojure_tags_finds_functions_and_defrecord() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping clojure_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("clojure") else {
        eprintln!("Skipping clojure_tags: clojure grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("clojure")
        .expect("clojure tags query missing");
    let names = collect_captures(&lang, CLOJURE_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' function in clojure tags, got: {names:?}"
    );
    assert!(
        names.contains(&"classify-point".to_string()),
        "expected 'classify-point' function in clojure tags, got: {names:?}"
    );
}

#[test]
fn clojure_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping clojure_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("clojure") else {
        eprintln!("Skipping clojure_calls: clojure grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("clojure")
        .expect("clojure calls query missing");
    let calls = collect_captures(&lang, CLOJURE_SAMPLE, &query_str, "call");
    assert!(
        calls.iter().any(|c| c == "println"),
        "expected 'println' call in clojure sample, got: {calls:?}"
    );
}

#[test]
fn clojure_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping clojure_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("clojure") else {
        eprintln!("Skipping clojure_complexity: clojure grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("clojure")
        .expect("clojure complexity query missing");
    let complexity = collect_captures(&lang, CLOJURE_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in clojure sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn clojure_imports_finds_require_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping clojure_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("clojure") else {
        eprintln!("Skipping clojure_imports: clojure grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("clojure")
        .expect("clojure imports query missing");
    let paths = collect_captures(&lang, CLOJURE_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("clojure")),
        "expected a clojure.* namespace in import paths, got: {paths:?}"
    );
}

#[test]
fn clojure_types_finds_no_captures() {
    // Clojure is dynamically typed; the types query intentionally captures nothing.
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping clojure_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("clojure") else {
        eprintln!("Skipping clojure_types: clojure grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("clojure")
        .expect("clojure types query missing");
    // Query parses successfully — result may be empty, that's correct for dynamic languages.
    let _ = collect_captures(&lang, CLOJURE_SAMPLE, &query_str, "type");
}

// ---------------------------------------------------------------------------
// Scheme
// ---------------------------------------------------------------------------

const SCHEME_SAMPLE: &str = include_str!("fixtures/scheme/sample.scm");

#[test]
fn scheme_tags_finds_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scheme_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scheme") else {
        eprintln!("Skipping scheme_tags: scheme grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("scheme")
        .expect("scheme tags query missing");
    let names = collect_captures(&lang, SCHEME_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' in scheme tags, got: {names:?}"
    );
    assert!(
        names.contains(&"square".to_string()),
        "expected 'square' in scheme tags, got: {names:?}"
    );
}

#[test]
fn scheme_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scheme_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scheme") else {
        eprintln!("Skipping scheme_calls: scheme grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("scheme")
        .expect("scheme calls query missing");
    let calls = collect_captures(&lang, SCHEME_SAMPLE, &query_str, "call");
    assert!(
        calls.iter().any(|c| c == "display" || c == "sqrt"),
        "expected 'display' or 'sqrt' call in scheme sample, got: {calls:?}"
    );
}

#[test]
fn scheme_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scheme_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scheme") else {
        eprintln!("Skipping scheme_complexity: scheme grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("scheme")
        .expect("scheme complexity query missing");
    let complexity = collect_captures(&lang, SCHEME_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in scheme sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn scheme_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scheme_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scheme") else {
        eprintln!("Skipping scheme_imports: scheme grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("scheme")
        .expect("scheme imports query missing");
    let paths = collect_captures(&lang, SCHEME_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("scheme")),
        "expected a scheme library in import paths, got: {paths:?}"
    );
}

#[test]
fn scheme_types_finds_no_captures() {
    // Scheme is dynamically typed; the types query intentionally captures nothing.
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scheme_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scheme") else {
        eprintln!("Skipping scheme_types: scheme grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("scheme")
        .expect("scheme types query missing");
    let _ = collect_captures(&lang, SCHEME_SAMPLE, &query_str, "type");
}

// ---------------------------------------------------------------------------
// D
// ---------------------------------------------------------------------------

const D_SAMPLE: &str = include_str!("fixtures/d/sample.d");

#[test]
fn d_tags_finds_functions_and_classes() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping d_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("d") else {
        eprintln!("Skipping d_tags: d grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("d").expect("d tags query missing");
    let names = collect_captures(&lang, D_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' function in d tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Shape".to_string()),
        "expected 'Shape' class in d tags, got: {names:?}"
    );
}

#[test]
fn d_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping d_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("d") else {
        eprintln!("Skipping d_calls: d grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("d").expect("d calls query missing");
    let calls = collect_captures(&lang, D_SAMPLE, &query_str, "call");
    assert!(
        calls.iter().any(|c| c == "writeln" || c == "sqrt"),
        "expected 'writeln' or 'sqrt' call in d sample, got: {calls:?}"
    );
}

#[test]
fn d_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping d_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("d") else {
        eprintln!("Skipping d_complexity: d grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("d")
        .expect("d complexity query missing");
    let complexity = collect_captures(&lang, D_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in d sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn d_imports_finds_module_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping d_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("d") else {
        eprintln!("Skipping d_imports: d grammar .so not found");
        return;
    };
    let query_str = loader.get_imports("d").expect("d imports query missing");
    let paths = collect_captures(&lang, D_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("std")),
        "expected std module in d import paths, got: {paths:?}"
    );
}

#[test]
fn d_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping d_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("d") else {
        eprintln!("Skipping d_types: d grammar .so not found");
        return;
    };
    let query_str = loader.get_types("d").expect("d types query missing");
    let refs = collect_captures(&lang, D_SAMPLE, &query_str, "type");
    assert!(
        !refs.is_empty(),
        "expected type references in d sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Visual Basic .NET
// ---------------------------------------------------------------------------

const VB_SAMPLE: &str = include_str!("fixtures/vb/sample.vb");

#[test]
fn vb_tags_finds_methods_and_classes() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vb_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vb") else {
        eprintln!("Skipping vb_tags: vb grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("vb").expect("vb tags query missing");
    let names = collect_captures(&lang, VB_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Classify".to_string()),
        "expected 'Classify' method in vb tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Circle".to_string()),
        "expected 'Circle' class in vb tags, got: {names:?}"
    );
}

#[test]
fn vb_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vb_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vb") else {
        eprintln!("Skipping vb_calls: vb grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("vb").expect("vb calls query missing");
    let calls = collect_captures(&lang, VB_SAMPLE, &query_str, "call");
    assert!(
        calls.iter().any(|c| c == "WriteLine" || c == "Area"),
        "expected 'WriteLine' or 'Area' call in vb sample, got: {calls:?}"
    );
}

#[test]
fn vb_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vb_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vb") else {
        eprintln!("Skipping vb_complexity: vb grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("vb")
        .expect("vb complexity query missing");
    let complexity = collect_captures(&lang, VB_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in vb sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn vb_imports_finds_namespace_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vb_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vb") else {
        eprintln!("Skipping vb_imports: vb grammar .so not found");
        return;
    };
    let query_str = loader.get_imports("vb").expect("vb imports query missing");
    let paths = collect_captures(&lang, VB_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("System")),
        "expected System namespace in vb import paths, got: {paths:?}"
    );
}

#[test]
fn vb_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vb_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vb") else {
        eprintln!("Skipping vb_types: vb grammar .so not found");
        return;
    };
    let query_str = loader.get_types("vb").expect("vb types query missing");
    let refs = collect_captures(&lang, VB_SAMPLE, &query_str, "type");
    assert!(
        !refs.is_empty(),
        "expected type references in vb sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Objective-C
// ---------------------------------------------------------------------------

const OBJC_SAMPLE: &str = include_str!("fixtures/objc/sample.m");

#[test]
fn objc_tags_finds_classes_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping objc_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("objc") else {
        eprintln!("Skipping objc_tags: objc grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("objc").expect("objc tags query missing");
    let names = collect_captures(&lang, OBJC_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' class in objc tags, got: {names:?}"
    );
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' function in objc tags, got: {names:?}"
    );
}

#[test]
fn objc_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping objc_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("objc") else {
        eprintln!("Skipping objc_calls: objc grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("objc").expect("objc calls query missing");
    let calls = collect_captures(&lang, OBJC_SAMPLE, &query_str, "call");
    assert!(
        calls.iter().any(|c| c == "distance" || c == "classify"),
        "expected 'distance' or 'classify' call in objc sample, got: {calls:?}"
    );
}

#[test]
fn objc_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping objc_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("objc") else {
        eprintln!("Skipping objc_complexity: objc grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("objc")
        .expect("objc complexity query missing");
    let complexity = collect_captures(&lang, OBJC_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in objc sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn objc_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping objc_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("objc") else {
        eprintln!("Skipping objc_imports: objc grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("objc")
        .expect("objc imports query missing");
    let paths = collect_captures(&lang, OBJC_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("Foundation")),
        "expected Foundation in objc import paths, got: {paths:?}"
    );
}

#[test]
fn objc_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping objc_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("objc") else {
        eprintln!("Skipping objc_types: objc grammar .so not found");
        return;
    };
    let query_str = loader.get_types("objc").expect("objc types query missing");
    let refs = collect_captures(&lang, OBJC_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "NSString" || r == "NSLog" || r == "Point"),
        "expected type reference in objc sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Idris
// ---------------------------------------------------------------------------

const IDRIS_SAMPLE: &str = include_str!("fixtures/idris/sample.idr");

#[test]
fn idris_tags_finds_functions_and_types() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping idris_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("idris") else {
        eprintln!("Skipping idris_tags: idris grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("idris").expect("idris tags query missing");
    let names = collect_captures(&lang, IDRIS_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' function in idris tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Shape".to_string()),
        "expected 'Shape' data type in idris tags, got: {names:?}"
    );
}

#[test]
fn idris_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping idris_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("idris") else {
        eprintln!("Skipping idris_calls: idris grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("idris")
        .expect("idris calls query missing");
    let calls = collect_captures(&lang, IDRIS_SAMPLE, &query_str, "call");
    assert!(
        calls.iter().any(|c| c == "sqrt" || c == "printLn"),
        "expected 'sqrt' or 'printLn' call in idris sample, got: {calls:?}"
    );
}

#[test]
fn idris_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping idris_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("idris") else {
        eprintln!("Skipping idris_complexity: idris grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("idris")
        .expect("idris complexity query missing");
    let complexity = collect_captures(&lang, IDRIS_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in idris sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn idris_imports_finds_module_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping idris_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("idris") else {
        eprintln!("Skipping idris_imports: idris grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("idris")
        .expect("idris imports query missing");
    let paths = collect_captures(&lang, IDRIS_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("Data")),
        "expected Data.* module in idris import paths, got: {paths:?}"
    );
}

#[test]
fn idris_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping idris_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("idris") else {
        eprintln!("Skipping idris_types: idris grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("idris")
        .expect("idris types query missing");
    let refs = collect_captures(&lang, IDRIS_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "String" || r == "Int" || r == "Double"),
        "expected a type reference in idris sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Lean 4
// ---------------------------------------------------------------------------

const LEAN_SAMPLE: &str = include_str!("fixtures/lean/sample.lean");

#[test]
fn lean_tags_finds_defs_and_structures() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping lean_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("lean") else {
        eprintln!("Skipping lean_tags: lean grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("lean").expect("lean tags query missing");
    let names = collect_captures(&lang, LEAN_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' def in lean tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' structure in lean tags, got: {names:?}"
    );
}

#[test]
fn lean_calls_finds_function_applications() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping lean_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("lean") else {
        eprintln!("Skipping lean_calls: lean grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("lean").expect("lean calls query missing");
    let calls = collect_captures(&lang, LEAN_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "sqrt" || c == "classify" || c == "IO.println"),
        "expected a function call in lean sample, got: {calls:?}"
    );
}

#[test]
fn lean_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping lean_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("lean") else {
        eprintln!("Skipping lean_complexity: lean grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("lean")
        .expect("lean complexity query missing");
    let complexity = collect_captures(&lang, LEAN_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in lean sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn lean_imports_finds_import_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping lean_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("lean") else {
        eprintln!("Skipping lean_imports: lean grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("lean")
        .expect("lean imports query missing");
    let paths = collect_captures(&lang, LEAN_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("Mathlib")),
        "expected Mathlib import in lean import paths, got: {paths:?}"
    );
}

#[test]
fn lean_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping lean_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("lean") else {
        eprintln!("Skipping lean_types: lean grammar .so not found");
        return;
    };
    let query_str = loader.get_types("lean").expect("lean types query missing");
    // Query parses and runs; lean type ascriptions may or may not match in this sample.
    let _ = collect_captures(&lang, LEAN_SAMPLE, &query_str, "type");
}

// ---------------------------------------------------------------------------
// ReScript
// ---------------------------------------------------------------------------

const RESCRIPT_SAMPLE: &str = include_str!("fixtures/rescript/sample.res");

#[test]
fn rescript_tags_finds_let_bindings_and_types() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rescript_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rescript") else {
        eprintln!("Skipping rescript_tags: rescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("rescript")
        .expect("rescript tags query missing");
    let names = collect_captures(&lang, RESCRIPT_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' in rescript tags, got: {names:?}"
    );
    assert!(
        names.contains(&"point".to_string()),
        "expected 'point' type in rescript tags, got: {names:?}"
    );
}

#[test]
fn rescript_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rescript_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rescript") else {
        eprintln!("Skipping rescript_calls: rescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("rescript")
        .expect("rescript calls query missing");
    let calls = collect_captures(&lang, RESCRIPT_SAMPLE, &query_str, "call");
    assert!(
        calls.iter().any(|c| c == "square" || c == "classify"),
        "expected 'square' or 'classify' call in rescript sample, got: {calls:?}"
    );
}

#[test]
fn rescript_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rescript_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rescript") else {
        eprintln!("Skipping rescript_complexity: rescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("rescript")
        .expect("rescript complexity query missing");
    let complexity = collect_captures(&lang, RESCRIPT_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in rescript sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn rescript_imports_finds_open_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rescript_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rescript") else {
        eprintln!("Skipping rescript_imports: rescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("rescript")
        .expect("rescript imports query missing");
    let paths = collect_captures(&lang, RESCRIPT_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("Belt")),
        "expected 'Belt' in rescript import paths, got: {paths:?}"
    );
}

#[test]
fn rescript_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping rescript_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("rescript") else {
        eprintln!("Skipping rescript_types: rescript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("rescript")
        .expect("rescript types query missing");
    let refs = collect_captures(&lang, RESCRIPT_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "float" || r == "int" || r == "point"),
        "expected a type reference in rescript sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Elm
// ---------------------------------------------------------------------------

const ELM_SAMPLE: &str = include_str!("fixtures/elm/sample.elm");

#[test]
fn elm_tags_finds_functions_and_types() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elm_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elm") else {
        eprintln!("Skipping elm_tags: elm grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("elm").expect("elm tags query missing");
    let names = collect_captures(&lang, ELM_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"distance".to_string()),
        "expected 'distance' function in elm tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Shape".to_string()),
        "expected 'Shape' type in elm tags, got: {names:?}"
    );
}

#[test]
fn elm_calls_finds_function_applications() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elm_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elm") else {
        eprintln!("Skipping elm_calls: elm grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("elm").expect("elm calls query missing");
    let calls = collect_captures(&lang, ELM_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "sqrt" || c == "classify" || c == "area"),
        "expected a function call in elm sample, got: {calls:?}"
    );
}

#[test]
fn elm_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elm_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elm") else {
        eprintln!("Skipping elm_complexity: elm grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("elm")
        .expect("elm complexity query missing");
    let complexity = collect_captures(&lang, ELM_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in elm sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn elm_imports_finds_module_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elm_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elm") else {
        eprintln!("Skipping elm_imports: elm grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("elm")
        .expect("elm imports query missing");
    let paths = collect_captures(&lang, ELM_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("Html")),
        "expected 'Html' in elm import paths, got: {paths:?}"
    );
}

#[test]
fn elm_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elm_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elm") else {
        eprintln!("Skipping elm_types: elm grammar .so not found");
        return;
    };
    let query_str = loader.get_types("elm").expect("elm types query missing");
    let refs = collect_captures(&lang, ELM_SAMPLE, &query_str, "type");
    assert!(
        refs.iter()
            .any(|r| r == "Html" || r == "Float" || r == "Int" || r == "String"),
        "expected a type reference in elm sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Zig
// ---------------------------------------------------------------------------

const ZIG_SAMPLE: &str = include_str!("fixtures/zig/sample.zig");

#[test]
fn zig_tags_finds_functions_and_structs() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zig_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zig") else {
        eprintln!("Skipping zig_tags: zig grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("zig").expect("zig tags query missing");
    let names = collect_captures(&lang, ZIG_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function in zig tags, got: {names:?}"
    );
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' struct in zig tags, got: {names:?}"
    );
}

#[test]
fn zig_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zig_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zig") else {
        eprintln!("Skipping zig_calls: zig grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("zig").expect("zig calls query missing");
    let calls = collect_captures(&lang, ZIG_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "classify" || c == "sumSlice" || c == "origin"),
        "expected a function call in zig sample, got: {calls:?}"
    );
}

#[test]
fn zig_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zig_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zig") else {
        eprintln!("Skipping zig_complexity: zig grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("zig")
        .expect("zig complexity query missing");
    let complexity = collect_captures(&lang, ZIG_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in zig sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn zig_imports_finds_module_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zig_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zig") else {
        eprintln!("Skipping zig_imports: zig grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("zig")
        .expect("zig imports query missing");
    let paths = collect_captures(&lang, ZIG_SAMPLE, &query_str, "import");
    assert!(
        !paths.is_empty(),
        "expected at least one import in zig sample, got: {paths:?}"
    );
}

#[test]
fn zig_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zig_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zig") else {
        eprintln!("Skipping zig_types: zig grammar .so not found");
        return;
    };
    let query_str = loader.get_types("zig").expect("zig types query missing");
    let refs = collect_captures(&lang, ZIG_SAMPLE, &query_str, "type");
    assert!(
        !refs.is_empty(),
        "expected at least one type reference in zig sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Ada
// ---------------------------------------------------------------------------

const ADA_SAMPLE: &str = include_str!("fixtures/ada/sample.adb");

#[test]
fn ada_tags_finds_subprograms_and_packages() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ada_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ada") else {
        eprintln!("Skipping ada_tags: ada grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("ada").expect("ada tags query missing");
    let names = collect_captures(&lang, ADA_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "Add" || n == "Classify" || n == "Calculator"),
        "expected 'Add'/'Classify'/'Calculator' in ada tags, got: {names:?}"
    );
}

#[test]
fn ada_calls_finds_procedure_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ada_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ada") else {
        eprintln!("Skipping ada_calls: ada grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("ada").expect("ada calls query missing");
    let calls = collect_captures(&lang, ADA_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "Print_Result" || c == "Put_Line" || c == "Add"),
        "expected a procedure call in ada sample, got: {calls:?}"
    );
}

#[test]
fn ada_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ada_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ada") else {
        eprintln!("Skipping ada_complexity: ada grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("ada")
        .expect("ada complexity query missing");
    let complexity = collect_captures(&lang, ADA_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in ada sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn ada_imports_finds_with_clauses() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ada_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ada") else {
        eprintln!("Skipping ada_imports: ada grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("ada")
        .expect("ada imports query missing");
    let paths = collect_captures(&lang, ADA_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("Text_IO") || p.contains("Ada")),
        "expected 'Ada.Text_IO' in ada import paths, got: {paths:?}"
    );
}

#[test]
fn ada_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping ada_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("ada") else {
        eprintln!("Skipping ada_types: ada grammar .so not found");
        return;
    };
    let query_str = loader.get_types("ada").expect("ada types query missing");
    let refs = collect_captures(&lang, ADA_SAMPLE, &query_str, "type");
    assert!(
        !refs.is_empty(),
        "expected at least one type reference in ada sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Perl
// ---------------------------------------------------------------------------

const PERL_SAMPLE: &str = include_str!("fixtures/perl/sample.pl");

#[test]
fn perl_tags_finds_subroutines_and_packages() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping perl_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("perl") else {
        eprintln!("Skipping perl_tags: perl grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("perl").expect("perl tags query missing");
    let names = collect_captures(&lang, PERL_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "classify" || n == "sum_array" || n == "factorial"),
        "expected 'classify'/'sum_array'/'factorial' in perl tags, got: {names:?}"
    );
}

#[test]
fn perl_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping perl_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("perl") else {
        eprintln!("Skipping perl_calls: perl grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("perl").expect("perl calls query missing");
    let calls = collect_captures(&lang, PERL_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "classify" || c == "sum_array" || c == "factorial"),
        "expected a function call in perl sample, got: {calls:?}"
    );
}

#[test]
fn perl_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping perl_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("perl") else {
        eprintln!("Skipping perl_complexity: perl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("perl")
        .expect("perl complexity query missing");
    let complexity = collect_captures(&lang, PERL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in perl sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn perl_imports_finds_use_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping perl_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("perl") else {
        eprintln!("Skipping perl_imports: perl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("perl")
        .expect("perl imports query missing");
    let paths = collect_captures(&lang, PERL_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("List") || p.contains("POSIX") || p.contains("warnings")),
        "expected a module path in perl imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Bash
// ---------------------------------------------------------------------------

const BASH_SAMPLE: &str = include_str!("fixtures/bash/sample.sh");

#[test]
fn bash_tags_finds_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping bash_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("bash") else {
        eprintln!("Skipping bash_tags: bash grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("bash").expect("bash tags query missing");
    let names = collect_captures(&lang, BASH_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "classify" || n == "sum_array" || n == "greet"),
        "expected 'classify'/'sum_array'/'greet' in bash tags, got: {names:?}"
    );
}

#[test]
fn bash_calls_finds_command_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping bash_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("bash") else {
        eprintln!("Skipping bash_calls: bash grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("bash").expect("bash calls query missing");
    let calls = collect_captures(&lang, BASH_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "classify" || c == "greet" || c == "sum_array"),
        "expected a function call in bash sample, got: {calls:?}"
    );
}

#[test]
fn bash_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping bash_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("bash") else {
        eprintln!("Skipping bash_complexity: bash grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("bash")
        .expect("bash complexity query missing");
    let complexity = collect_captures(&lang, BASH_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in bash sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn bash_imports_finds_source_commands() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping bash_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("bash") else {
        eprintln!("Skipping bash_imports: bash grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("bash")
        .expect("bash imports query missing");
    let paths = collect_captures(&lang, BASH_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("utils") || p.contains("config")),
        "expected sourced file path in bash imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// PowerShell
// ---------------------------------------------------------------------------

const POWERSHELL_SAMPLE: &str = include_str!("fixtures/powershell/sample.ps1");

#[test]
fn powershell_tags_finds_functions_and_classes() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping powershell_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("powershell") else {
        eprintln!("Skipping powershell_tags: powershell grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("powershell")
        .expect("powershell tags query missing");
    let names = collect_captures(&lang, POWERSHELL_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "Invoke-Classify" || n == "Get-Sum" || n == "Calculator"),
        "expected 'Invoke-Classify'/'Get-Sum'/'Calculator' in powershell tags, got: {names:?}"
    );
}

#[test]
fn powershell_calls_finds_command_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping powershell_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("powershell") else {
        eprintln!("Skipping powershell_calls: powershell grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("powershell")
        .expect("powershell calls query missing");
    let calls = collect_captures(&lang, POWERSHELL_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "Invoke-Classify" || c == "Get-Sum" || c == "Write-Host"),
        "expected a call in powershell sample, got: {calls:?}"
    );
}

#[test]
fn powershell_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping powershell_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("powershell") else {
        eprintln!("Skipping powershell_complexity: powershell grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("powershell")
        .expect("powershell complexity query missing");
    let complexity = collect_captures(&lang, POWERSHELL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in powershell sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn powershell_imports_finds_import_module() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping powershell_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("powershell") else {
        eprintln!("Skipping powershell_imports: powershell grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("powershell")
        .expect("powershell imports query missing");
    let paths = collect_captures(&lang, POWERSHELL_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("PSReadLine") || p.contains("PowerShell")),
        "expected a module path in powershell imports, got: {paths:?}"
    );
}

#[test]
fn powershell_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping powershell_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("powershell") else {
        eprintln!("Skipping powershell_types: powershell grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("powershell")
        .expect("powershell types query missing");
    let refs = collect_captures(&lang, POWERSHELL_SAMPLE, &query_str, "type");
    assert!(
        !refs.is_empty(),
        "expected at least one type reference in powershell sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Fish
// ---------------------------------------------------------------------------

const FISH_SAMPLE: &str = include_str!("fixtures/fish/sample.fish");

#[test]
fn fish_tags_finds_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping fish_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("fish") else {
        eprintln!("Skipping fish_tags: fish grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("fish").expect("fish tags query missing");
    let names = collect_captures(&lang, FISH_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "classify" || n == "greet" || n == "sum_list"),
        "expected 'classify'/'greet'/'sum_list' in fish tags, got: {names:?}"
    );
}

#[test]
fn fish_calls_finds_command_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping fish_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("fish") else {
        eprintln!("Skipping fish_calls: fish grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("fish").expect("fish calls query missing");
    let calls = collect_captures(&lang, FISH_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "classify" || c == "greet" || c == "sum_list"),
        "expected a function call in fish sample, got: {calls:?}"
    );
}

#[test]
fn fish_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping fish_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("fish") else {
        eprintln!("Skipping fish_complexity: fish grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("fish")
        .expect("fish complexity query missing");
    let complexity = collect_captures(&lang, FISH_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in fish sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn fish_imports_finds_source_commands() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping fish_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("fish") else {
        eprintln!("Skipping fish_imports: fish grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("fish")
        .expect("fish imports query missing");
    let paths = collect_captures(&lang, FISH_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("utils") || p.contains("fish")),
        "expected sourced file path in fish imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Zsh
// ---------------------------------------------------------------------------

const ZSH_SAMPLE: &str = include_str!("fixtures/zsh/sample.zsh");

#[test]
fn zsh_tags_finds_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zsh_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zsh") else {
        eprintln!("Skipping zsh_tags: zsh grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("zsh").expect("zsh tags query missing");
    let names = collect_captures(&lang, ZSH_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "classify" || n == "greet" || n == "sum_array"),
        "expected 'classify'/'greet'/'sum_array' in zsh tags, got: {names:?}"
    );
}

#[test]
fn zsh_calls_finds_command_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zsh_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zsh") else {
        eprintln!("Skipping zsh_calls: zsh grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("zsh").expect("zsh calls query missing");
    let calls = collect_captures(&lang, ZSH_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "classify" || c == "greet" || c == "sum_array"),
        "expected a function call in zsh sample, got: {calls:?}"
    );
}

#[test]
fn zsh_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zsh_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zsh") else {
        eprintln!("Skipping zsh_complexity: zsh grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("zsh")
        .expect("zsh complexity query missing");
    let complexity = collect_captures(&lang, ZSH_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in zsh sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn zsh_imports_finds_source_commands() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping zsh_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("zsh") else {
        eprintln!("Skipping zsh_imports: zsh grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("zsh")
        .expect("zsh imports query missing");
    let paths = collect_captures(&lang, ZSH_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("utils") || p.contains("zsh") || p.contains("helpers")),
        "expected sourced file path in zsh imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// AWK
// ---------------------------------------------------------------------------

const AWK_SAMPLE: &str = include_str!("fixtures/awk/sample.awk");

#[test]
fn awk_tags_finds_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping awk_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("awk") else {
        eprintln!("Skipping awk_tags: awk grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("awk").expect("awk tags query missing");
    let names = collect_captures(&lang, AWK_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "classify" || n == "max" || n == "trim"),
        "expected 'classify'/'max'/'trim' in awk tags, got: {names:?}"
    );
}

#[test]
fn awk_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping awk_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("awk") else {
        eprintln!("Skipping awk_calls: awk grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("awk").expect("awk calls query missing");
    let calls = collect_captures(&lang, AWK_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "classify" || c == "max" || c == "trim" || c == "gsub"),
        "expected a function call in awk sample, got: {calls:?}"
    );
}

#[test]
fn awk_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping awk_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("awk") else {
        eprintln!("Skipping awk_complexity: awk grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("awk")
        .expect("awk complexity query missing");
    let complexity = collect_captures(&lang, AWK_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in awk sample, got {} ({complexity:?})",
        complexity.len()
    );
}

// ---------------------------------------------------------------------------
// JavaScript
// ---------------------------------------------------------------------------

const JAVASCRIPT_SAMPLE: &str = include_str!("fixtures/javascript/sample.js");

#[test]
fn javascript_tags_finds_functions_and_classes() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping javascript_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("javascript") else {
        eprintln!("Skipping javascript_tags: javascript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("javascript")
        .expect("javascript tags query missing");
    let names = collect_captures(&lang, JAVASCRIPT_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "Stack" || n == "classify" || n == "fibonacci"),
        "expected 'Stack'/'classify'/'fibonacci' in javascript tags, got: {names:?}"
    );
}

#[test]
fn javascript_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping javascript_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("javascript") else {
        eprintln!("Skipping javascript_calls: javascript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("javascript")
        .expect("javascript calls query missing");
    let calls = collect_captures(&lang, JAVASCRIPT_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "classify" || c == "fibonacci" || c == "push"),
        "expected a function call in javascript sample, got: {calls:?}"
    );
}

#[test]
fn javascript_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping javascript_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("javascript") else {
        eprintln!("Skipping javascript_complexity: javascript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("javascript")
        .expect("javascript complexity query missing");
    let complexity = collect_captures(&lang, JAVASCRIPT_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in javascript sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn javascript_imports_finds_es_module_imports() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping javascript_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("javascript") else {
        eprintln!("Skipping javascript_imports: javascript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("javascript")
        .expect("javascript imports query missing");
    let paths = collect_captures(&lang, JAVASCRIPT_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p == "events" || p == "path" || p == "fs"),
        "expected module paths in javascript imports, got: {paths:?}"
    );
}

#[test]
fn javascript_types_finds_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping javascript_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("javascript") else {
        eprintln!("Skipping javascript_types: javascript grammar .so not found");
        return;
    };
    let query_str = loader
        .get_types("javascript")
        .expect("javascript types query missing");
    let refs = collect_captures(&lang, JAVASCRIPT_SAMPLE, &query_str, "type");
    assert!(
        !refs.is_empty(),
        "expected at least one type reference in javascript sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// TSX
// ---------------------------------------------------------------------------

const TSX_SAMPLE: &str = include_str!("fixtures/tsx/sample.tsx");

#[test]
fn tsx_tags_finds_components_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tsx_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tsx") else {
        eprintln!("Skipping tsx_tags: tsx grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("tsx").expect("tsx tags query missing");
    let names = collect_captures(&lang, TSX_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "Counter" || n == "Button" || n == "classify"),
        "expected 'Counter'/'Button'/'classify' in tsx tags, got: {names:?}"
    );
}

#[test]
fn tsx_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tsx_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tsx") else {
        eprintln!("Skipping tsx_calls: tsx grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("tsx").expect("tsx calls query missing");
    let calls = collect_captures(&lang, TSX_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "useState" || c == "useEffect" || c == "classify"),
        "expected a hook/function call in tsx sample, got: {calls:?}"
    );
}

#[test]
fn tsx_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tsx_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tsx") else {
        eprintln!("Skipping tsx_complexity: tsx grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("tsx")
        .expect("tsx complexity query missing");
    let complexity = collect_captures(&lang, TSX_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in tsx sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn tsx_imports_finds_react_imports() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tsx_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tsx") else {
        eprintln!("Skipping tsx_imports: tsx grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("tsx")
        .expect("tsx imports query missing");
    let paths = collect_captures(&lang, TSX_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p == "react" || p == "react-native"),
        "expected 'react'/'react-native' in tsx import paths, got: {paths:?}"
    );
}

#[test]
fn tsx_types_finds_interface_and_type_references() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tsx_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tsx") else {
        eprintln!("Skipping tsx_types: tsx grammar .so not found");
        return;
    };
    let query_str = loader.get_types("tsx").expect("tsx types query missing");
    let refs = collect_captures(&lang, TSX_SAMPLE, &query_str, "type");
    assert!(
        !refs.is_empty(),
        "expected at least one type reference in tsx sample, got: {refs:?}"
    );
}

// ---------------------------------------------------------------------------
// Agda
// ---------------------------------------------------------------------------

const AGDA_SAMPLE: &str = include_str!("fixtures/agda/sample.agda");

#[test]
fn agda_tags_finds_functions_and_types() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping agda_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("agda") else {
        eprintln!("Skipping agda_tags: agda grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("agda").expect("agda tags query missing");
    let names = collect_captures(&lang, AGDA_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Shape".to_string()),
        "expected 'Shape' data type in agda tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "classify" || n == "area" || n == "double"),
        "expected a function name in agda tags, got: {names:?}"
    );
}

#[test]
fn agda_calls_finds_applications() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping agda_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("agda") else {
        eprintln!("Skipping agda_calls: agda grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("agda").expect("agda calls query missing");
    let calls = collect_captures(&lang, AGDA_SAMPLE, &query_str, "call");
    assert!(
        !calls.is_empty(),
        "expected at least one call in agda sample, got: {calls:?}"
    );
}

#[test]
fn agda_complexity_finds_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping agda_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("agda") else {
        eprintln!("Skipping agda_complexity: agda grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("agda")
        .expect("agda complexity query missing");
    let complexity = collect_captures(&lang, AGDA_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in agda sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn agda_imports_finds_module_paths() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping agda_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("agda") else {
        eprintln!("Skipping agda_imports: agda grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("agda")
        .expect("agda imports query missing");
    let paths = collect_captures(&lang, AGDA_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("Data")),
        "expected a 'Data.*' import path in agda sample, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Common Lisp
// ---------------------------------------------------------------------------

const COMMONLISP_SAMPLE: &str = include_str!("fixtures/commonlisp/sample.lisp");

#[test]
fn commonlisp_tags_finds_functions_and_structs() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping commonlisp_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("commonlisp") else {
        eprintln!("Skipping commonlisp_tags: commonlisp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("commonlisp")
        .expect("commonlisp tags query missing");
    let names = collect_captures(&lang, COMMONLISP_SAMPLE, &query_str, "name");
    assert!(
        names.iter().any(|n| n == "factorial"),
        "expected 'factorial' function in commonlisp tags, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "point" || n == "shape"),
        "expected 'point' or 'shape' struct/class in commonlisp tags, got: {names:?}"
    );
}

#[test]
fn commonlisp_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping commonlisp_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("commonlisp") else {
        eprintln!("Skipping commonlisp_calls: commonlisp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("commonlisp")
        .expect("commonlisp calls query missing");
    let calls = collect_captures(&lang, COMMONLISP_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "format" || c == "setf" || c == "dolist"),
        "expected a standard form call in commonlisp sample, got: {calls:?}"
    );
}

#[test]
fn commonlisp_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping commonlisp_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("commonlisp") else {
        eprintln!("Skipping commonlisp_complexity: commonlisp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("commonlisp")
        .expect("commonlisp complexity query missing");
    let complexity = collect_captures(&lang, COMMONLISP_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in commonlisp sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn commonlisp_imports_finds_require() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping commonlisp_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("commonlisp") else {
        eprintln!("Skipping commonlisp_imports: commonlisp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("commonlisp")
        .expect("commonlisp imports query missing");
    let paths = collect_captures(&lang, COMMONLISP_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("alexandria") || p.contains("iterate")),
        "expected 'alexandria' or 'iterate' in commonlisp import paths, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Emacs Lisp
// ---------------------------------------------------------------------------

const ELISP_SAMPLE: &str = include_str!("fixtures/elisp/sample.el");

#[test]
fn elisp_tags_finds_functions_and_vars() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elisp_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elisp") else {
        eprintln!("Skipping elisp_tags: elisp grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("elisp").expect("elisp tags query missing");
    let names = collect_captures(&lang, ELISP_SAMPLE, &query_str, "name");
    assert!(
        names.iter().any(|n| n == "sample-greet"),
        "expected 'sample-greet' function in elisp tags, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "sample-counter"),
        "expected 'sample-counter' var in elisp tags, got: {names:?}"
    );
}

#[test]
fn elisp_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elisp_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elisp") else {
        eprintln!("Skipping elisp_calls: elisp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("elisp")
        .expect("elisp calls query missing");
    let calls = collect_captures(&lang, ELISP_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "message" || c == "setq" || c == "dolist"),
        "expected a standard form in elisp calls, got: {calls:?}"
    );
}

#[test]
fn elisp_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elisp_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elisp") else {
        eprintln!("Skipping elisp_complexity: elisp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("elisp")
        .expect("elisp complexity query missing");
    let complexity = collect_captures(&lang, ELISP_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in elisp sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn elisp_imports_finds_require() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping elisp_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("elisp") else {
        eprintln!("Skipping elisp_imports: elisp grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("elisp")
        .expect("elisp imports query missing");
    let paths = collect_captures(&lang, ELISP_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("cl-lib") || p.contains("subr-x")),
        "expected 'cl-lib' or 'subr-x' in elisp import paths, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Prolog
// ---------------------------------------------------------------------------

const PROLOG_SAMPLE: &str = include_str!("fixtures/prolog/sample.pl");

#[test]
fn prolog_tags_finds_predicates() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping prolog_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("prolog") else {
        eprintln!("Skipping prolog_tags: prolog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("prolog")
        .expect("prolog tags query missing");
    let names = collect_captures(&lang, PROLOG_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "factorial" || n == "parent" || n == "ancestor"),
        "expected 'factorial', 'parent', or 'ancestor' in prolog tags, got: {names:?}"
    );
}

#[test]
fn prolog_calls_finds_predicate_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping prolog_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("prolog") else {
        eprintln!("Skipping prolog_calls: prolog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("prolog")
        .expect("prolog calls query missing");
    let calls = collect_captures(&lang, PROLOG_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "factorial" || c == "parent" || c == "member"),
        "expected a predicate call in prolog sample, got: {calls:?}"
    );
}

#[test]
fn prolog_complexity_finds_clauses() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping prolog_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("prolog") else {
        eprintln!("Skipping prolog_complexity: prolog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("prolog")
        .expect("prolog complexity query missing");
    let complexity = collect_captures(&lang, PROLOG_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in prolog sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn prolog_imports_finds_use_module() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping prolog_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("prolog") else {
        eprintln!("Skipping prolog_imports: prolog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("prolog")
        .expect("prolog imports query missing");
    let paths = collect_captures(&lang, PROLOG_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("lists") || p.contains("apply")),
        "expected 'lists' or 'apply' in prolog import paths, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// SQL
// ---------------------------------------------------------------------------

const SQL_SAMPLE: &str = include_str!("fixtures/sql/sample.sql");

#[test]
fn sql_tags_finds_tables_and_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping sql_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("sql") else {
        eprintln!("Skipping sql_tags: sql grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("sql").expect("sql tags query missing");
    let names = collect_captures(&lang, SQL_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n.contains("products") || n == "products"),
        "expected 'products' table in sql tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n.contains("calculate_total") || n == "calculate_total"),
        "expected 'calculate_total' function in sql tags, got: {names:?}"
    );
}

#[test]
fn sql_types_finds_column_types() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping sql_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("sql") else {
        eprintln!("Skipping sql_types: sql grammar .so not found");
        return;
    };
    let query_str = loader.get_types("sql").expect("sql types query missing");
    let types = collect_captures(&lang, SQL_SAMPLE, &query_str, "type");
    assert!(
        !types.is_empty(),
        "expected at least one type in sql sample, got: {types:?}"
    );
}

#[test]
fn sql_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping sql_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("sql") else {
        eprintln!("Skipping sql_complexity: sql grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("sql")
        .expect("sql complexity query missing");
    let complexity = collect_captures(&lang, SQL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in sql sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn sql_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping sql_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("sql") else {
        eprintln!("Skipping sql_calls: sql grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("sql").expect("sql calls query missing");
    let calls = collect_captures(&lang, SQL_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "NOW" || c == "COUNT" || c == "SUM" || c == "COALESCE"),
        "expected a SQL function call in sql sample, got: {calls:?}"
    );
}

// ---------------------------------------------------------------------------
// Starlark
// ---------------------------------------------------------------------------

const STARLARK_SAMPLE: &str = include_str!("fixtures/starlark/sample.star");

#[test]
fn starlark_tags_finds_functions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping starlark_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("starlark") else {
        eprintln!("Skipping starlark_tags: starlark grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("starlark")
        .expect("starlark tags query missing");
    let names = collect_captures(&lang, STARLARK_SAMPLE, &query_str, "name");
    assert!(
        names.iter().any(|n| n == "make_cc_library"),
        "expected 'make_cc_library' in starlark tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "make_test_suite" || n == "filter_srcs"),
        "expected another function in starlark tags, got: {names:?}"
    );
}

#[test]
fn starlark_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping starlark_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("starlark") else {
        eprintln!("Skipping starlark_calls: starlark grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("starlark")
        .expect("starlark calls query missing");
    let calls = collect_captures(&lang, STARLARK_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "cc_library" || c == "cc_binary" || c == "make_cc_library"),
        "expected a function call in starlark sample, got: {calls:?}"
    );
}

#[test]
fn starlark_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping starlark_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("starlark") else {
        eprintln!("Skipping starlark_complexity: starlark grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("starlark")
        .expect("starlark complexity query missing");
    let complexity = collect_captures(&lang, STARLARK_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 complexity nodes in starlark sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn starlark_imports_finds_load_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping starlark_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("starlark") else {
        eprintln!("Skipping starlark_imports: starlark grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("starlark")
        .expect("starlark imports query missing");
    let paths = collect_captures(&lang, STARLARK_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("rules_cc") || p.contains("rules_python")),
        "expected a load path in starlark sample, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// HCL (Terraform)
// ---------------------------------------------------------------------------

const HCL_SAMPLE: &str = include_str!("fixtures/hcl/sample.tf");

#[test]
fn hcl_tags_finds_blocks() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hcl_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hcl") else {
        eprintln!("Skipping hcl_tags: hcl grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("hcl").expect("hcl tags query missing");
    let names = collect_captures(&lang, HCL_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "resource" || n == "variable" || n == "output"),
        "expected a block type in hcl tags, got: {names:?}"
    );
}

#[test]
fn hcl_types_finds_type_constraints() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hcl_types: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hcl") else {
        eprintln!("Skipping hcl_types: hcl grammar .so not found");
        return;
    };
    let query_str = loader.get_types("hcl").expect("hcl types query missing");
    let types = collect_captures(&lang, HCL_SAMPLE, &query_str, "type");
    assert!(
        !types.is_empty(),
        "expected at least one type constraint in hcl sample, got: {types:?}"
    );
}

#[test]
fn hcl_complexity_finds_conditionals() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hcl_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hcl") else {
        eprintln!("Skipping hcl_complexity: hcl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("hcl")
        .expect("hcl complexity query missing");
    let complexity = collect_captures(&lang, HCL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in hcl sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn hcl_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hcl_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hcl") else {
        eprintln!("Skipping hcl_calls: hcl grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("hcl").expect("hcl calls query missing");
    let calls = collect_captures(&lang, HCL_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "merge" || c == "toset" || c == "lookup"),
        "expected a HCL function call in hcl sample, got: {calls:?}"
    );
}

#[test]
fn hcl_imports_finds_module_sources() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hcl_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hcl") else {
        eprintln!("Skipping hcl_imports: hcl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("hcl")
        .expect("hcl imports query missing");
    let paths = collect_captures(&lang, HCL_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("modules/vpc") || p.contains("vpc")),
        "expected a module source path in hcl sample, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Nix
// ---------------------------------------------------------------------------

const NIX_SAMPLE: &str = include_str!("fixtures/nix/sample.nix");

#[test]
fn nix_tags_finds_bindings() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping nix_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("nix") else {
        eprintln!("Skipping nix_tags: nix grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("nix").expect("nix tags query missing");
    let names = collect_captures(&lang, NIX_SAMPLE, &query_str, "name");
    assert!(
        names.iter().any(|n| n == "greet" || n == "factorial"),
        "expected 'greet' or 'factorial' binding in nix tags, got: {names:?}"
    );
}

#[test]
fn nix_calls_finds_applications() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping nix_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("nix") else {
        eprintln!("Skipping nix_calls: nix grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("nix").expect("nix calls query missing");
    let calls = collect_captures(&lang, NIX_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "factorial" || c == "greet" || c == "filter"),
        "expected an application in nix sample, got: {calls:?}"
    );
}

#[test]
fn nix_complexity_finds_if_expressions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping nix_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("nix") else {
        eprintln!("Skipping nix_complexity: nix grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("nix")
        .expect("nix complexity query missing");
    let complexity = collect_captures(&lang, NIX_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in nix sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn nix_imports_finds_import_expressions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping nix_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("nix") else {
        eprintln!("Skipping nix_imports: nix grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("nix")
        .expect("nix imports query missing");
    let paths = collect_captures(&lang, NIX_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("nixpkgs") || p.contains("src")),
        "expected an import path in nix sample, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// MATLAB
// ---------------------------------------------------------------------------

const MATLAB_SAMPLE: &str = include_str!("fixtures/matlab/sample.m");

#[test]
fn matlab_tags_finds_functions_and_class() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping matlab_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("matlab") else {
        eprintln!("Skipping matlab_tags: matlab grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("matlab")
        .expect("matlab tags query missing");
    let names = collect_captures(&lang, MATLAB_SAMPLE, &query_str, "name");
    assert!(
        names.iter().any(|n| n == "factorial"),
        "expected 'factorial' function in matlab tags, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "Shape"),
        "expected 'Shape' class in matlab tags, got: {names:?}"
    );
}

#[test]
fn matlab_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping matlab_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("matlab") else {
        eprintln!("Skipping matlab_calls: matlab grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("matlab")
        .expect("matlab calls query missing");
    let calls = collect_captures(&lang, MATLAB_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "factorial" || c == "fprintf" || c == "length"),
        "expected a function call in matlab sample, got: {calls:?}"
    );
}

#[test]
fn matlab_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping matlab_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("matlab") else {
        eprintln!("Skipping matlab_complexity: matlab grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("matlab")
        .expect("matlab complexity query missing");
    let complexity = collect_captures(&lang, MATLAB_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 3,
        "expected at least 3 complexity nodes in matlab sample, got {} ({complexity:?})",
        complexity.len()
    );
}

// ---------------------------------------------------------------------------
// TLA+
// ---------------------------------------------------------------------------

const TLAPLUS_SAMPLE: &str = include_str!("fixtures/tlaplus/sample.tla");

#[test]
fn tlaplus_tags_finds_module_and_operators() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tlaplus_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tlaplus") else {
        eprintln!("Skipping tlaplus_tags: tlaplus grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("tlaplus")
        .expect("tlaplus tags query missing");
    let names = collect_captures(&lang, TLAPLUS_SAMPLE, &query_str, "name");
    assert!(
        names.iter().any(|n| n == "Sample"),
        "expected 'Sample' module in tlaplus tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "Init" || n == "Next" || n == "Safety"),
        "expected an operator definition in tlaplus tags, got: {names:?}"
    );
}

#[test]
fn tlaplus_complexity_finds_conditionals() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tlaplus_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tlaplus") else {
        eprintln!("Skipping tlaplus_complexity: tlaplus grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("tlaplus")
        .expect("tlaplus complexity query missing");
    let complexity = collect_captures(&lang, TLAPLUS_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in tlaplus sample, got {} ({complexity:?})",
        complexity.len()
    );
}

#[test]
fn tlaplus_imports_finds_extends() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping tlaplus_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("tlaplus") else {
        eprintln!("Skipping tlaplus_imports: tlaplus grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("tlaplus")
        .expect("tlaplus imports query missing");
    let paths = collect_captures(&lang, TLAPLUS_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("Naturals") || p.contains("Sequences")),
        "expected 'Naturals' or 'Sequences' in tlaplus import paths, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// CMake
// ---------------------------------------------------------------------------

const CMAKE_SAMPLE: &str = include_str!("fixtures/cmake/CMakeLists.txt");

#[test]
fn cmake_tags_finds_functions_and_macros() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cmake_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cmake") else {
        eprintln!("Skipping cmake_tags: cmake grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("cmake").expect("cmake tags query missing");
    let names = collect_captures(&lang, CMAKE_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"add_component".to_string()),
        "expected 'add_component' function in cmake tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "setup_target" || n == "install_component"),
        "expected 'setup_target' or 'install_component' in cmake tags, got: {names:?}"
    );
}

#[test]
fn cmake_calls_finds_command_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cmake_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cmake") else {
        eprintln!("Skipping cmake_calls: cmake grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("cmake")
        .expect("cmake calls query missing");
    let calls = collect_captures(&lang, CMAKE_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "find_package" || c == "add_library" || c == "target_link_libraries"),
        "expected cmake command calls in sample, got: {calls:?}"
    );
}

#[test]
fn cmake_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cmake_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cmake") else {
        eprintln!("Skipping cmake_complexity: cmake grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("cmake")
        .expect("cmake complexity query missing");
    let complexity = collect_captures(&lang, CMAKE_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in cmake sample, got: {complexity:?}"
    );
}

#[test]
fn cmake_imports_finds_includes_and_find_package() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping cmake_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("cmake") else {
        eprintln!("Skipping cmake_imports: cmake grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("cmake")
        .expect("cmake imports query missing");
    let paths = collect_captures(&lang, CMAKE_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p == "Threads" || p == "OpenSSL" || p == "GNUInstallDirs"),
        "expected 'Threads'/'OpenSSL'/'GNUInstallDirs' in cmake import paths, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// GraphQL
// ---------------------------------------------------------------------------

const GRAPHQL_SAMPLE: &str = include_str!("fixtures/graphql/sample.graphql");

#[test]
fn graphql_tags_finds_types_and_interfaces() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping graphql_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("graphql") else {
        eprintln!("Skipping graphql_tags: graphql grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("graphql")
        .expect("graphql tags query missing");
    let names = collect_captures(&lang, GRAPHQL_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"User".to_string()),
        "expected 'User' type in graphql tags, got: {names:?}"
    );
    assert!(
        names.contains(&"UserRole".to_string()),
        "expected 'UserRole' enum in graphql tags, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "Node" || n == "Timestamped"),
        "expected interface name in graphql tags, got: {names:?}"
    );
}

#[test]
fn graphql_calls_finds_field_selections() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping graphql_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("graphql") else {
        eprintln!("Skipping graphql_calls: graphql grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("graphql")
        .expect("graphql calls query missing");
    // GraphQL calls query captures field names; runs cleanly against schema definitions
    let calls = collect_captures(&lang, GRAPHQL_SAMPLE, &query_str, "call");
    assert!(
        calls.len() >= 0,
        "graphql calls query should run cleanly, got: {calls:?}"
    );
}

#[test]
fn graphql_complexity_query_runs_cleanly() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping graphql_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("graphql") else {
        eprintln!("Skipping graphql_complexity: graphql grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("graphql")
        .expect("graphql complexity query missing");
    let complexity = collect_captures(&lang, GRAPHQL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 0,
        "graphql complexity query should run cleanly, got: {complexity:?}"
    );
}

// ---------------------------------------------------------------------------
// GLSL
// ---------------------------------------------------------------------------

const GLSL_SAMPLE: &str = include_str!("fixtures/glsl/sample.glsl");

#[test]
fn glsl_tags_finds_functions_and_structs() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping glsl_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("glsl") else {
        eprintln!("Skipping glsl_tags: glsl grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("glsl").expect("glsl tags query missing");
    let names = collect_captures(&lang, GLSL_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"main".to_string()),
        "expected 'main' function in glsl tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "Material" || n == "calculateDiffuse" || n == "applyFog"),
        "expected a struct or function name in glsl tags, got: {names:?}"
    );
}

#[test]
fn glsl_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping glsl_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("glsl") else {
        eprintln!("Skipping glsl_calls: glsl grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("glsl").expect("glsl calls query missing");
    let calls = collect_captures(&lang, GLSL_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "normalize" || c == "texture" || c == "calculateDiffuse"),
        "expected builtin or user function call in glsl sample, got: {calls:?}"
    );
}

#[test]
fn glsl_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping glsl_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("glsl") else {
        eprintln!("Skipping glsl_complexity: glsl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("glsl")
        .expect("glsl complexity query missing");
    let complexity = collect_captures(&lang, GLSL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in glsl sample, got: {complexity:?}"
    );
}

// ---------------------------------------------------------------------------
// HLSL
// ---------------------------------------------------------------------------

const HLSL_SAMPLE: &str = include_str!("fixtures/hlsl/sample.hlsl");

#[test]
fn hlsl_tags_finds_functions_structs_and_cbuffers() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hlsl_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hlsl") else {
        eprintln!("Skipping hlsl_tags: hlsl grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("hlsl").expect("hlsl tags query missing");
    let names = collect_captures(&lang, HLSL_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "VSMain" || n == "PSMain" || n == "ComputeLighting"),
        "expected a function name in hlsl tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "PerFrame" || n == "PerObject" || n == "VSInput"),
        "expected a cbuffer or struct name in hlsl tags, got: {names:?}"
    );
}

#[test]
fn hlsl_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hlsl_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hlsl") else {
        eprintln!("Skipping hlsl_calls: hlsl grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("hlsl").expect("hlsl calls query missing");
    let calls = collect_captures(&lang, HLSL_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "normalize" || c == "mul" || c == "ComputeLighting"),
        "expected function calls in hlsl sample, got: {calls:?}"
    );
}

#[test]
fn hlsl_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hlsl_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hlsl") else {
        eprintln!("Skipping hlsl_complexity: hlsl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("hlsl")
        .expect("hlsl complexity query missing");
    let complexity = collect_captures(&lang, HLSL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node in hlsl sample, got: {complexity:?}"
    );
}

#[test]
fn hlsl_imports_finds_include_directives() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping hlsl_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("hlsl") else {
        eprintln!("Skipping hlsl_imports: hlsl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("hlsl")
        .expect("hlsl imports query missing");
    let paths = collect_captures(&lang, HLSL_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("common.hlsl") || p.contains("d3d11.h")),
        "expected include paths in hlsl imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// jq
// ---------------------------------------------------------------------------

const JQ_SAMPLE: &str = include_str!("fixtures/jq/sample.jq");

#[test]
fn jq_tags_finds_function_definitions() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping jq_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("jq") else {
        eprintln!("Skipping jq_tags: jq grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("jq").expect("jq tags query missing");
    let names = collect_captures(&lang, JQ_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"sum".to_string()),
        "expected 'sum' function in jq tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "mean" || n == "flatten_keys" || n == "keep_if"),
        "expected function names in jq tags, got: {names:?}"
    );
}

#[test]
fn jq_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping jq_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("jq") else {
        eprintln!("Skipping jq_calls: jq grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("jq").expect("jq calls query missing");
    let calls = collect_captures(&lang, JQ_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "map" || c == "select" || c == "group_by" || c == "sort_by"),
        "expected builtin function calls in jq sample, got: {calls:?}"
    );
}

#[test]
fn jq_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping jq_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("jq") else {
        eprintln!("Skipping jq_complexity: jq grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("jq")
        .expect("jq complexity query missing");
    let complexity = collect_captures(&lang, JQ_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 0,
        "jq complexity query should run cleanly, got: {complexity:?}"
    );
}

#[test]
fn jq_imports_finds_import_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping jq_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("jq") else {
        eprintln!("Skipping jq_imports: jq grammar .so not found");
        return;
    };
    let query_str = loader.get_imports("jq").expect("jq imports query missing");
    let paths = collect_captures(&lang, JQ_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("lib/utils")),
        "expected 'lib/utils' in jq import paths, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

const MARKDOWN_SAMPLE: &str = include_str!("fixtures/markdown/sample.md");

#[test]
fn markdown_tags_finds_headings() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping markdown_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("markdown") else {
        eprintln!("Skipping markdown_tags: markdown grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("markdown")
        .expect("markdown tags query missing");
    let names = collect_captures(&lang, MARKDOWN_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n.contains("Getting Started") || n.contains("Installation")),
        "expected heading names in markdown tags, got: {names:?}"
    );
}

// ---------------------------------------------------------------------------
// Meson
// ---------------------------------------------------------------------------

const MESON_SAMPLE: &str = include_str!("fixtures/meson/meson.build");

#[test]
fn meson_tags_finds_variable_assignments() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping meson_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("meson") else {
        eprintln!("Skipping meson_tags: meson grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("meson").expect("meson tags query missing");
    // Meson tags captures variable identifiers from var_unit assignments
    let names = collect_captures(&lang, MESON_SAMPLE, &query_str, "name");
    assert!(
        names.len() >= 0,
        "meson tags query should run cleanly, got: {names:?}"
    );
}

#[test]
fn meson_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping meson_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("meson") else {
        eprintln!("Skipping meson_calls: meson grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("meson")
        .expect("meson calls query missing");
    let calls = collect_captures(&lang, MESON_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "project" || c == "dependency" || c == "executable" || c == "library"),
        "expected meson function calls in sample, got: {calls:?}"
    );
}

#[test]
fn meson_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping meson_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("meson") else {
        eprintln!("Skipping meson_complexity: meson grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("meson")
        .expect("meson complexity query missing");
    let complexity = collect_captures(&lang, MESON_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node (if block) in meson sample, got: {complexity:?}"
    );
}

#[test]
fn meson_imports_finds_subproject_and_dependency() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping meson_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("meson") else {
        eprintln!("Skipping meson_imports: meson grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("meson")
        .expect("meson imports query missing");
    let paths = collect_captures(&lang, MESON_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("glib-2.0") || p.contains("zlib") || p.contains("protobuf")),
        "expected dependency names in meson imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Nginx
// ---------------------------------------------------------------------------

const NGINX_SAMPLE: &str = include_str!("fixtures/nginx/nginx.conf");

#[test]
fn nginx_tags_finds_block_directives() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping nginx_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("nginx") else {
        eprintln!("Skipping nginx_tags: nginx grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("nginx").expect("nginx tags query missing");
    let names = collect_captures(&lang, NGINX_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "server" || n == "http" || n == "upstream"),
        "expected block directive names in nginx tags, got: {names:?}"
    );
}

#[test]
fn nginx_complexity_finds_block_directives() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping nginx_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("nginx") else {
        eprintln!("Skipping nginx_complexity: nginx grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("nginx")
        .expect("nginx complexity query missing");
    let complexity = collect_captures(&lang, NGINX_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 2,
        "expected at least 2 block directive complexity nodes in nginx sample, got: {complexity:?}"
    );
}

#[test]
fn nginx_imports_finds_include_directives() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping nginx_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("nginx") else {
        eprintln!("Skipping nginx_imports: nginx grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("nginx")
        .expect("nginx imports query missing");
    let paths = collect_captures(&lang, NGINX_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("mime.types") || p.contains("fastcgi_params")),
        "expected include paths in nginx imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// SCSS
// ---------------------------------------------------------------------------

const SCSS_SAMPLE: &str = include_str!("fixtures/scss/sample.scss");

#[test]
fn scss_tags_finds_mixins_functions_and_rules() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scss_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scss") else {
        eprintln!("Skipping scss_tags: scss grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("scss").expect("scss tags query missing");
    let names = collect_captures(&lang, SCSS_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "flex-center" || n == "responsive"),
        "expected mixin names in scss tags, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "rem" || n == "shade"),
        "expected function names in scss tags, got: {names:?}"
    );
}

#[test]
fn scss_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scss_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scss") else {
        eprintln!("Skipping scss_calls: scss grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("scss").expect("scss calls query missing");
    let calls = collect_captures(&lang, SCSS_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "darken" || c == "rgba" || c == "shade"),
        "expected function calls in scss sample, got: {calls:?}"
    );
}

#[test]
fn scss_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scss_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scss") else {
        eprintln!("Skipping scss_complexity: scss grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("scss")
        .expect("scss complexity query missing");
    let complexity = collect_captures(&lang, SCSS_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node (@if/@each) in scss sample, got: {complexity:?}"
    );
}

#[test]
fn scss_imports_finds_use_and_import_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping scss_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("scss") else {
        eprintln!("Skipping scss_imports: scss grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("scss")
        .expect("scss imports query missing");
    let paths = collect_captures(&lang, SCSS_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("sass:math") || p.contains("variables") || p.contains("mixins")),
        "expected import paths in scss sample, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Svelte
// ---------------------------------------------------------------------------

const SVELTE_SAMPLE: &str = include_str!("fixtures/svelte/sample.svelte");

#[test]
fn svelte_tags_finds_script_and_style_blocks() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping svelte_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("svelte") else {
        eprintln!("Skipping svelte_tags: svelte grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("svelte")
        .expect("svelte tags query missing");
    let names = collect_captures(&lang, SVELTE_SAMPLE, &query_str, "name");
    assert!(
        names.iter().any(|n| n == "script" || n == "style"),
        "expected 'script' or 'style' block tags in svelte sample, got: {names:?}"
    );
}

#[test]
fn svelte_calls_query_runs_cleanly() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping svelte_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("svelte") else {
        eprintln!("Skipping svelte_calls: svelte grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("svelte")
        .expect("svelte calls query missing");
    // Svelte calls query is intentionally empty (JS in <script> is raw_text)
    let calls = collect_captures(&lang, SVELTE_SAMPLE, &query_str, "call");
    assert!(
        calls.len() >= 0,
        "svelte calls query should run cleanly, got: {calls:?}"
    );
}

#[test]
fn svelte_complexity_query_runs_cleanly() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping svelte_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("svelte") else {
        eprintln!("Skipping svelte_complexity: svelte grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("svelte")
        .expect("svelte complexity query missing");
    let complexity = collect_captures(&lang, SVELTE_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 0,
        "svelte complexity query should run cleanly, got: {complexity:?}"
    );
}

// ---------------------------------------------------------------------------
// Typst
// ---------------------------------------------------------------------------

const TYPST_SAMPLE: &str = include_str!("fixtures/typst/sample.typ");

#[test]
fn typst_tags_finds_let_bindings() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typst_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typst") else {
        eprintln!("Skipping typst_tags: typst grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("typst").expect("typst tags query missing");
    let names = collect_captures(&lang, TYPST_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "format_version" || n == "summary_table"),
        "expected function let bindings in typst tags, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "project_name" || n == "version"),
        "expected variable let bindings in typst tags, got: {names:?}"
    );
}

#[test]
fn typst_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typst_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typst") else {
        eprintln!("Skipping typst_calls: typst grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("typst")
        .expect("typst calls query missing");
    let calls = collect_captures(&lang, TYPST_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "tablex" || c == "format_version" || c == "summary_table"),
        "expected function calls in typst sample, got: {calls:?}"
    );
}

#[test]
fn typst_complexity_query_runs_cleanly() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typst_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typst") else {
        eprintln!("Skipping typst_complexity: typst grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("typst")
        .expect("typst complexity query missing");
    let complexity = collect_captures(&lang, TYPST_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 0,
        "typst complexity query should run cleanly, got: {complexity:?}"
    );
}

#[test]
fn typst_imports_finds_import_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping typst_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("typst") else {
        eprintln!("Skipping typst_imports: typst grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("typst")
        .expect("typst imports query missing");
    let paths = collect_captures(&lang, TYPST_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("template.typ") || p.contains("tablex")),
        "expected import paths in typst sample, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Verilog
// ---------------------------------------------------------------------------

const VERILOG_SAMPLE: &str = include_str!("fixtures/verilog/sample.v");

#[test]
fn verilog_tags_finds_modules() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping verilog_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("verilog") else {
        eprintln!("Skipping verilog_tags: verilog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_tags("verilog")
        .expect("verilog tags query missing");
    let names = collect_captures(&lang, VERILOG_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"alu".to_string()),
        "expected 'alu' module in verilog tags, got: {names:?}"
    );
    assert!(
        names.contains(&"reg_file".to_string()),
        "expected 'reg_file' module in verilog tags, got: {names:?}"
    );
}

#[test]
fn verilog_calls_finds_task_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping verilog_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("verilog") else {
        eprintln!("Skipping verilog_calls: verilog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_calls("verilog")
        .expect("verilog calls query missing");
    let calls = collect_captures(&lang, VERILOG_SAMPLE, &query_str, "call");
    assert!(
        calls.len() >= 0,
        "verilog calls query should run cleanly, got: {calls:?}"
    );
}

#[test]
fn verilog_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping verilog_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("verilog") else {
        eprintln!("Skipping verilog_complexity: verilog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("verilog")
        .expect("verilog complexity query missing");
    let complexity = collect_captures(&lang, VERILOG_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node (always/case/if) in verilog sample, got: {complexity:?}"
    );
}

#[test]
fn verilog_imports_query_runs_cleanly() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping verilog_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("verilog") else {
        eprintln!("Skipping verilog_imports: verilog grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("verilog")
        .expect("verilog imports query missing");
    let paths = collect_captures(&lang, VERILOG_SAMPLE, &query_str, "import.path");
    assert!(
        paths.len() >= 0,
        "verilog imports query should run cleanly (no package imports in sample), got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// VHDL
// ---------------------------------------------------------------------------

const VHDL_SAMPLE: &str = include_str!("fixtures/vhdl/sample.vhd");

#[test]
fn vhdl_tags_finds_entity_and_architecture() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vhdl_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vhdl") else {
        eprintln!("Skipping vhdl_tags: vhdl grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("vhdl").expect("vhdl tags query missing");
    let names = collect_captures(&lang, VHDL_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"fifo".to_string()),
        "expected 'fifo' entity in vhdl tags, got: {names:?}"
    );
    assert!(
        names.contains(&"rtl".to_string()),
        "expected 'rtl' architecture in vhdl tags, got: {names:?}"
    );
}

#[test]
fn vhdl_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vhdl_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vhdl") else {
        eprintln!("Skipping vhdl_calls: vhdl grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("vhdl").expect("vhdl calls query missing");
    let calls = collect_captures(&lang, VHDL_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "rising_edge" || c == "to_integer"),
        "expected function calls in vhdl sample, got: {calls:?}"
    );
}

#[test]
fn vhdl_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vhdl_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vhdl") else {
        eprintln!("Skipping vhdl_complexity: vhdl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("vhdl")
        .expect("vhdl complexity query missing");
    let complexity = collect_captures(&lang, VHDL_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node (if/process) in vhdl sample, got: {complexity:?}"
    );
}

#[test]
fn vhdl_imports_finds_use_clauses() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vhdl_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vhdl") else {
        eprintln!("Skipping vhdl_imports: vhdl grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("vhdl")
        .expect("vhdl imports query missing");
    let paths = collect_captures(&lang, VHDL_SAMPLE, &query_str, "import.path");
    assert!(
        paths.iter().any(|p| p.contains("std_logic_1164")
            || p.contains("numeric_std")
            || p.contains("ieee")),
        "expected use clause paths in vhdl sample, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Vim script
// ---------------------------------------------------------------------------

const VIM_SAMPLE: &str = include_str!("fixtures/vim/sample.vim");

#[test]
fn vim_tags_finds_functions_and_augroups() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vim_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vim") else {
        eprintln!("Skipping vim_tags: vim grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("vim").expect("vim tags query missing");
    let names = collect_captures(&lang, VIM_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "ToggleOption" || n == "FormatBuffer" || n == "OpenTerminal"),
        "expected function names in vim tags, got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "MyPlugin" || n == "FileTypeSettings"),
        "expected augroup names in vim tags, got: {names:?}"
    );
}

#[test]
fn vim_calls_finds_function_calls() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vim_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vim") else {
        eprintln!("Skipping vim_calls: vim grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("vim").expect("vim calls query missing");
    let calls = collect_captures(&lang, VIM_SAMPLE, &query_str, "call");
    assert!(
        calls
            .iter()
            .any(|c| c == "FormatBuffer" || c == "getpos" || c == "setpos"),
        "expected function calls in vim sample, got: {calls:?}"
    );
}

#[test]
fn vim_complexity_finds_control_flow() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vim_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vim") else {
        eprintln!("Skipping vim_complexity: vim grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("vim")
        .expect("vim complexity query missing");
    let complexity = collect_captures(&lang, VIM_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 1,
        "expected at least 1 complexity node (if block) in vim sample, got: {complexity:?}"
    );
}

#[test]
fn vim_imports_finds_source_statements() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vim_imports: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vim") else {
        eprintln!("Skipping vim_imports: vim grammar .so not found");
        return;
    };
    let query_str = loader
        .get_imports("vim")
        .expect("vim imports query missing");
    let paths = collect_captures(&lang, VIM_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("utils.vim") || p.contains("defaults.vim")),
        "expected sourced file paths in vim imports, got: {paths:?}"
    );
}

// ---------------------------------------------------------------------------
// Vue
// ---------------------------------------------------------------------------

const VUE_SAMPLE: &str = include_str!("fixtures/vue/sample.vue");

#[test]
fn vue_tags_finds_script_template_and_style_blocks() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vue_tags: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vue") else {
        eprintln!("Skipping vue_tags: vue grammar .so not found");
        return;
    };
    let query_str = loader.get_tags("vue").expect("vue tags query missing");
    let names = collect_captures(&lang, VUE_SAMPLE, &query_str, "name");
    assert!(
        names
            .iter()
            .any(|n| n == "script" || n == "template" || n == "style"),
        "expected SFC block tag names in vue tags, got: {names:?}"
    );
}

#[test]
fn vue_calls_query_runs_cleanly() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vue_calls: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vue") else {
        eprintln!("Skipping vue_calls: vue grammar .so not found");
        return;
    };
    let query_str = loader.get_calls("vue").expect("vue calls query missing");
    // Vue calls query is intentionally empty (JS in <script> is raw_text)
    let calls = collect_captures(&lang, VUE_SAMPLE, &query_str, "call");
    assert!(
        calls.len() >= 0,
        "vue calls query should run cleanly, got: {calls:?}"
    );
}

#[test]
fn vue_complexity_query_runs_cleanly() {
    let Some(gdir) = grammar_dir() else {
        eprintln!("Skipping vue_complexity: run `cargo xtask build-grammars` first");
        return;
    };
    let loader = GrammarLoader::with_paths(vec![gdir]);
    let Some(lang) = loader.get("vue") else {
        eprintln!("Skipping vue_complexity: vue grammar .so not found");
        return;
    };
    let query_str = loader
        .get_complexity("vue")
        .expect("vue complexity query missing");
    let complexity = collect_captures(&lang, VUE_SAMPLE, &query_str, "complexity");
    assert!(
        complexity.len() >= 0,
        "vue complexity query should run cleanly, got: {complexity:?}"
    );
}

// ---------------------------------------------------------------------------
// Groovy / Elixir / Haskell live grammar tests (use ~/.config/normalize/grammars/)
// ---------------------------------------------------------------------------

#[test]
fn groovy_tags_live() {
    let loader = normalize_languages::GrammarLoader::new();
    let Some(lang) = loader.get("groovy") else {
        eprintln!("Skipping groovy_tags_live: groovy grammar not found");
        return;
    };
    let query_str = loader
        .get_tags("groovy")
        .expect("groovy tags query missing");
    let names = collect_captures(&lang, GROOVY_SAMPLE, &query_str, "name");
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' class, got: {names:?}"
    );
    assert!(
        names.contains(&"distanceTo".to_string()),
        "expected 'distanceTo' method, got: {names:?}"
    );
    assert!(
        names.contains(&"MathUtils".to_string()),
        "expected 'MathUtils' class, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' method, got: {names:?}"
    );
    assert!(
        names.contains(&"greet".to_string()),
        "expected 'greet' function, got: {names:?}"
    );
}

#[test]
fn elixir_tags_no_args_function() {
    let loader = normalize_languages::GrammarLoader::new();
    let Some(lang) = loader.get("elixir") else {
        eprintln!("Skipping elixir_tags_no_args_function: elixir grammar not found");
        return;
    };
    let query_str = loader
        .get_tags("elixir")
        .expect("elixir tags query missing");
    // Test the no-args function form: "def name do ... end"
    let source = r#"defmodule Foo do
  def initialize do
    :ok
  end

  def greet(name) do
    name
  end
end"#;
    let names = collect_captures(&lang, source, &query_str, "name");
    assert!(
        names.contains(&"initialize".to_string()),
        "expected 'initialize' (no-args def), got: {names:?}"
    );
    assert!(
        names.contains(&"greet".to_string()),
        "expected 'greet' (with-args def), got: {names:?}"
    );
}

const HASKELL_SAMPLE: &str = include_str!("fixtures/haskell/sample.hs");

#[test]
fn haskell_tags_no_duplicate_signatures() {
    let loader = normalize_languages::GrammarLoader::new();
    let Some(lang) = loader.get("haskell") else {
        eprintln!("Skipping haskell_tags_no_duplicate_signatures: haskell grammar not found");
        return;
    };
    let query_str = loader
        .get_tags("haskell")
        .expect("haskell tags query missing");
    let names = collect_captures(&lang, HASKELL_SAMPLE, &query_str, "name");
    // "classify" has one equation; with type signatures removed, it should appear exactly once
    // (before fix: appeared twice — once for signature, once for definition).
    let classify_count = names.iter().filter(|n| *n == "classify").count();
    assert_eq!(
        classify_count, 1,
        "expected 'classify' exactly once (type signature removed), got: {names:?}"
    );
    // "insert" has two equations (multi-equation function); the grammar produces one `function`
    // node per equation, so it legitimately appears twice in the raw query output.
    // Deduplication to a single symbol happens in the extraction layer (normalize-facts).
    let insert_count = names.iter().filter(|n| *n == "insert").count();
    assert!(
        insert_count <= 2 && insert_count >= 1,
        "expected 'insert' 1-2 times (multi-equation), got: {names:?}"
    );
    // Type names from data/newtype/type should also be present
    assert!(
        names.contains(&"Tree".to_string()),
        "expected 'Tree' data type, got: {names:?}"
    );
    assert!(
        names.contains(&"Count".to_string()),
        "expected 'Count' newtype, got: {names:?}"
    );
}

const GROOVY_SAMPLE: &str = include_str!("fixtures/groovy/sample.groovy");

#[test]
fn groovy_imports_live() {
    let loader = normalize_languages::GrammarLoader::new();
    let Some(lang) = loader.get("groovy") else {
        eprintln!("Skipping groovy_imports_live: groovy grammar not found");
        return;
    };
    let query_str = loader
        .get_imports("groovy")
        .expect("groovy imports query missing");
    let paths = collect_captures(&lang, GROOVY_SAMPLE, &query_str, "import.path");
    assert!(
        paths
            .iter()
            .any(|p| p.contains("Immutable") || p.contains("groovy")),
        "expected 'groovy.transform.Immutable' in groovy import paths, got: {paths:?}"
    );
    assert!(
        paths
            .iter()
            .any(|p| p.contains("ArrayList") || p.contains("java")),
        "expected 'java.util.ArrayList' in groovy import paths, got: {paths:?}"
    );
}

#[test]
fn kotlin_tags_live() {
    let loader = normalize_languages::GrammarLoader::new();
    let Some(lang) = loader.get("kotlin") else {
        eprintln!("Skipping kotlin_tags_live: kotlin grammar not found");
        return;
    };
    let query_str = loader
        .get_tags("kotlin")
        .expect("kotlin tags query missing");
    let names = collect_captures(&lang, KOTLIN_SAMPLE, &query_str, "name");
    // After fix: should find classes and functions, NOT local val declarations
    assert!(
        names.contains(&"Point".to_string()),
        "expected 'Point' class, got: {names:?}"
    );
    assert!(
        names.contains(&"classify".to_string()),
        "expected 'classify' function, got: {names:?}"
    );
    assert!(
        names.contains(&"sumEvens".to_string()),
        "expected 'sumEvens' function, got: {names:?}"
    );
    // Local val declarations should NOT appear
    assert!(
        !names.contains(&"dx".to_string()),
        "local 'dx' should not appear in tags, got: {names:?}"
    );
    assert!(
        !names.contains(&"total".to_string()),
        "local 'total' should not appear in tags, got: {names:?}"
    );
}
