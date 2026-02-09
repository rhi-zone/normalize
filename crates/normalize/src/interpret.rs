//! Interpreted Datalog rule evaluation using ascent-interpreter.
//!
//! This module bridges normalize's Relations to the ascent-interpreter engine,
//! enabling users to write `.dl` files that run directly without compilation.
//!
//! # Rule File Format
//!
//! ```dl
//! # ---
//! # id = "circular-deps"
//! # message = "Circular dependency detected between modules"
//! # enabled = true
//! # ---
//!
//! relation reaches(String, String);
//! reaches(from, to) <-- import(from, to, _);
//! reaches(from, to) <-- import(from, mid, _), reaches(mid, to);
//!
//! warning("circular-deps", a) <-- reaches(a, b), reaches(b, a), if a < b;
//! ```
//!
//! # Convention
//!
//! Input relations are pre-populated from the index:
//! - `symbol(file: String, name: String, kind: String, line: u32)`
//! - `import(from_file: String, to_module: String, name: String)`
//! - `call(caller_file: String, caller_name: String, callee_name: String, line: u32)`
//!
//! Output relations are read as diagnostics:
//! - `warning(rule_id: String, message: String)` → produces warnings
//! - `error(rule_id: String, message: String)` → produces errors

use ascent_eval::{Engine, Value};
use ascent_ir::Program;
use ascent_syntax::AscentProgram;
use normalize_facts_rules_api::{Diagnostic, Relations};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Preamble declaring the built-in input relations.
/// Users don't need to declare these - they're always available.
const PREAMBLE: &str = r#"
relation symbol(String, String, String, u32);
relation import(String, String, String);
relation call(String, String, String, u32);
relation warning(String, String);
relation error(String, String);
"#;

// =============================================================================
// Rule types and loading
// =============================================================================

/// A Datalog fact rule definition.
#[derive(Debug)]
pub struct FactsRule {
    /// Unique identifier for this rule.
    pub id: String,
    /// The Datalog source (without frontmatter).
    pub source: String,
    /// Description for display when listing rules.
    pub message: String,
    /// Whether this rule is enabled.
    pub enabled: bool,
    /// Whether this is a builtin rule.
    pub builtin: bool,
    /// Source file path (empty for builtins).
    pub source_path: PathBuf,
}

/// A builtin rule definition (id + embedded content).
pub struct BuiltinFactsRule {
    pub id: &'static str,
    pub content: &'static str,
}

/// All embedded builtin rules.
const BUILTIN_RULES: &[BuiltinFactsRule] = &[
    BuiltinFactsRule {
        id: "circular-deps",
        content: include_str!("builtin_dl/circular_deps.dl"),
    },
    BuiltinFactsRule {
        id: "orphan-file",
        content: include_str!("builtin_dl/orphan_file.dl"),
    },
    BuiltinFactsRule {
        id: "self-import",
        content: include_str!("builtin_dl/self_import.dl"),
    },
    BuiltinFactsRule {
        id: "god-file",
        content: include_str!("builtin_dl/god_file.dl"),
    },
    BuiltinFactsRule {
        id: "fan-out",
        content: include_str!("builtin_dl/fan_out.dl"),
    },
    BuiltinFactsRule {
        id: "hub-file",
        content: include_str!("builtin_dl/hub_file.dl"),
    },
    BuiltinFactsRule {
        id: "duplicate-symbol",
        content: include_str!("builtin_dl/duplicate_symbol.dl"),
    },
];

/// Load all rules from all sources, merged by ID.
/// Order: builtins → ~/.config/moss/rules/ → .normalize/rules/
pub fn load_all_rules(project_root: &Path) -> Vec<FactsRule> {
    let mut rules_by_id: HashMap<String, FactsRule> = HashMap::new();

    // 1. Load embedded builtins
    for builtin in BUILTIN_RULES {
        if let Some(rule) = parse_rule_content(builtin.content, builtin.id, true) {
            rules_by_id.insert(rule.id.clone(), rule);
        }
    }

    // 2. Load user global rules (~/.config/moss/rules/)
    if let Some(config_dir) = dirs::config_dir() {
        let user_rules_dir = config_dir.join("moss").join("rules");
        for rule in load_rules_from_dir(&user_rules_dir) {
            rules_by_id.insert(rule.id.clone(), rule);
        }
    }

    // 3. Load project rules (.normalize/rules/)
    let project_rules_dir = project_root.join(".normalize").join("rules");
    for rule in load_rules_from_dir(&project_rules_dir) {
        rules_by_id.insert(rule.id.clone(), rule);
    }

    // Filter out disabled rules
    rules_by_id.into_values().filter(|r| r.enabled).collect()
}

/// Load rules from a directory (only `.dl` files).
fn load_rules_from_dir(rules_dir: &Path) -> Vec<FactsRule> {
    let mut rules = Vec::new();

    if !rules_dir.exists() {
        return rules;
    }

    let entries = match std::fs::read_dir(rules_dir) {
        Ok(e) => e,
        Err(_) => return rules,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "dl")
            && let Some(rule) = parse_rule_file(&path)
        {
            rules.push(rule);
        }
    }

    rules
}

/// Parse a rule file with TOML frontmatter.
fn parse_rule_file(path: &Path) -> Option<FactsRule> {
    let content = std::fs::read_to_string(path).ok()?;
    let default_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let mut rule = parse_rule_content(&content, default_id, false)?;
    rule.source_path = path.to_path_buf();
    Some(rule)
}

/// Parse rule content string with TOML frontmatter.
///
/// Frontmatter is delimited by `# ---` lines and contains TOML:
/// ```text
/// # ---
/// # id = "rule-id"
/// # message = "What this rule checks for"
/// # enabled = true
/// # ---
/// ```
pub fn parse_rule_content(content: &str, default_id: &str, is_builtin: bool) -> Option<FactsRule> {
    let lines: Vec<&str> = content.lines().collect();

    let mut in_frontmatter = false;
    let mut frontmatter_lines = Vec::new();
    let mut source_lines = Vec::new();

    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "# ---" {
            in_frontmatter = !in_frontmatter;
            continue;
        }

        if in_frontmatter {
            let fm_line = line.strip_prefix('#').unwrap_or(line).trim_start();
            frontmatter_lines.push(fm_line);
        } else if !frontmatter_lines.is_empty()
            || (frontmatter_lines.is_empty() && !trimmed.is_empty() && !trimmed.starts_with('#'))
        {
            source_lines.push(*line);
        }
    }

    let (frontmatter_str, source_str) = if frontmatter_lines.is_empty() {
        (String::new(), content.to_string())
    } else {
        (frontmatter_lines.join("\n"), source_lines.join("\n"))
    };

    let frontmatter: toml::Value = if frontmatter_str.is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        match toml::from_str(&frontmatter_str) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("Warning: invalid frontmatter: {}", e);
                return None;
            }
        }
    };

    let id = frontmatter
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_id.to_string());

    let message = frontmatter
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Datalog rule")
        .to_string();

    let enabled = frontmatter
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    Some(FactsRule {
        id,
        source: source_str.trim().to_string(),
        message,
        enabled,
        builtin: is_builtin,
        source_path: PathBuf::new(),
    })
}

// =============================================================================
// Execution
// =============================================================================

/// Error type for interpretation
#[derive(Debug)]
pub enum InterpretError {
    /// Failed to read the rules file
    Io(std::io::Error),
    /// Failed to parse the rules
    Parse(String),
}

impl std::fmt::Display for InterpretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpretError::Io(e) => write!(f, "Failed to read rules file: {}", e),
            InterpretError::Parse(e) => write!(f, "Failed to parse rules: {}", e),
        }
    }
}

impl std::error::Error for InterpretError {}

/// Run a `.dl` rules file against the given relations.
/// Returns diagnostics produced by the rules.
pub fn run_rules_file(
    path: &Path,
    relations: &Relations,
) -> Result<Vec<Diagnostic>, InterpretError> {
    let source = std::fs::read_to_string(path).map_err(InterpretError::Io)?;
    run_rules_source(&source, relations)
}

/// Run a FactsRule against the given relations.
pub fn run_rule(
    rule: &FactsRule,
    relations: &Relations,
) -> Result<Vec<Diagnostic>, InterpretError> {
    run_rules_source(&rule.source, relations)
}

/// Run rules from a source string against the given relations.
pub fn run_rules_source(
    source: &str,
    relations: &Relations,
) -> Result<Vec<Diagnostic>, InterpretError> {
    // Combine preamble with user rules
    let full_source = format!("{}\n{}", PREAMBLE, source);

    // Parse through ascent-syntax → IR
    let ast: AscentProgram =
        syn::parse_str(&full_source).map_err(|e| InterpretError::Parse(e.to_string()))?;
    let program = Program::from_ast(ast);

    // Create engine and populate facts
    let mut engine = Engine::new(&program);
    populate_facts(&mut engine, relations);

    // Run to fixpoint
    engine.run(&program);

    // Extract diagnostics from output relations
    Ok(extract_diagnostics(&engine))
}

/// Populate the engine with facts from Relations
fn populate_facts(engine: &mut Engine, relations: &Relations) {
    for sym in relations.symbols.iter() {
        engine.insert(
            "symbol",
            vec![
                Value::String(Rc::new(sym.file.to_string())),
                Value::String(Rc::new(sym.name.to_string())),
                Value::String(Rc::new(sym.kind.to_string())),
                Value::U32(sym.line),
            ],
        );
    }

    for imp in relations.imports.iter() {
        engine.insert(
            "import",
            vec![
                Value::String(Rc::new(imp.from_file.to_string())),
                Value::String(Rc::new(imp.to_module.to_string())),
                Value::String(Rc::new(imp.name.to_string())),
            ],
        );
    }

    for call in relations.calls.iter() {
        engine.insert(
            "call",
            vec![
                Value::String(Rc::new(call.caller_file.to_string())),
                Value::String(Rc::new(call.caller_name.to_string())),
                Value::String(Rc::new(call.callee_name.to_string())),
                Value::U32(call.line),
            ],
        );
    }
}

/// Extract diagnostics from the warning/error output relations
fn extract_diagnostics(engine: &Engine) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if let Some(warnings) = engine.relation("warning") {
        for tuple in warnings.iter() {
            if let [Value::String(rule_id), Value::String(message)] = tuple {
                diagnostics.push(Diagnostic::warning(rule_id, message));
            }
        }
    }

    if let Some(errors) = engine.relation("error") {
        for tuple in errors.iter() {
            if let [Value::String(rule_id), Value::String(message)] = tuple {
                diagnostics.push(Diagnostic::error(rule_id, message));
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_rules() {
        let relations = Relations::new();
        let result = run_rules_source("", &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_cycle_detection_interpreted() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "*");
        relations.add_import("b.py", "a.py", "*");

        let rules = r#"
            relation reaches(String, String);
            relation cycle(String, String);

            reaches(from, to) <-- import(from, to, _);
            reaches(from, to) <-- import(from, mid, _), reaches(mid, to);
            cycle(a, b) <-- reaches(a, b), reaches(b, a), if a < b;

            warning("circular-deps", a) <-- cycle(a, _);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].rule_id.as_str(), "circular-deps");
    }

    #[test]
    fn test_no_cycles() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "*");
        relations.add_import("b.py", "c.py", "*");

        let rules = r#"
            relation reaches(String, String);
            relation cycle(String, String);

            reaches(from, to) <-- import(from, to, _);
            reaches(from, to) <-- import(from, mid, _), reaches(mid, to);
            cycle(a, b) <-- reaches(a, b), reaches(b, a), if a < b;

            warning("circular-deps", a) <-- cycle(a, _);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_rule_content_with_frontmatter() {
        let content = r#"
# ---
# id = "test-rule"
# message = "A test rule"
# ---

relation foo(String);
warning("test-rule", x) <-- foo(x);
"#;

        let rule = parse_rule_content(content, "fallback-id", false).unwrap();
        assert_eq!(rule.id, "test-rule");
        assert_eq!(rule.message, "A test rule");
        assert!(rule.enabled);
        assert!(!rule.builtin);
        assert!(rule.source.contains("relation foo"));
    }

    #[test]
    fn test_parse_rule_content_without_frontmatter() {
        let content = "relation foo(String);\nwarning(\"x\", y) <-- foo(y);";

        let rule = parse_rule_content(content, "my-rule", false).unwrap();
        assert_eq!(rule.id, "my-rule");
        assert_eq!(rule.message, "Datalog rule");
        assert!(rule.source.contains("relation foo"));
    }

    #[test]
    fn test_parse_rule_content_disabled() {
        let content = r#"
# ---
# id = "disabled-rule"
# enabled = false
# ---

warning("x", "y") <-- symbol(_, _, _, _);
"#;

        let rule = parse_rule_content(content, "x", false).unwrap();
        assert!(!rule.enabled);
    }

    #[test]
    fn test_builtin_rules_parse() {
        for builtin in BUILTIN_RULES {
            let rule = parse_rule_content(builtin.content, builtin.id, true);
            assert!(rule.is_some(), "Failed to parse builtin: {}", builtin.id);
            let rule = rule.unwrap();
            assert!(rule.builtin);
            assert!(!rule.source.is_empty());
        }
    }

    #[test]
    fn test_negation() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "foo", "function", 1);
        relations.add_symbol("b.py", "bar", "function", 1);
        relations.add_call("a.py", "main", "foo", 5);
        // bar is never called

        let rules = r#"
            relation defined(String);
            relation called(String);
            relation uncalled(String);

            defined(name) <-- symbol(_, name, _, _);
            called(name) <-- call(_, _, name, _);
            uncalled(name) <-- defined(name), !called(name);

            warning("uncalled", name) <-- uncalled(name);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "bar");
    }

    #[test]
    fn test_string_comparison_in_if() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "MyClass", "class", 1);
        relations.add_symbol("a.py", "my_func", "function", 10);

        let rules = r#"
            relation func(String, String);
            func(file, name) <-- symbol(file, name, kind, _), if kind == "function";
            warning("func-found", name) <-- func(_, name);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "my_func");
    }

    #[test]
    fn test_aggregation_count() {
        let mut relations = Relations::new();
        relations.add_symbol("big.py", "a", "function", 1);
        relations.add_symbol("big.py", "b", "function", 2);
        relations.add_symbol("big.py", "c", "function", 3);
        relations.add_symbol("small.py", "x", "function", 1);

        let rules = r#"
            relation file_count(String, i32);
            file_count(file, c) <-- symbol(file, _, _, _), agg c = count() in symbol(file, _, _, _);
            warning("big-file", file) <-- file_count(file, c), if c > 2;
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "big.py");
    }

    #[test]
    fn test_run_builtin_cycle_detection() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "*");
        relations.add_import("b.py", "a.py", "*");

        let rule = find_builtin("circular-deps");
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].rule_id.as_str(), "circular-deps");
    }

    #[test]
    fn test_orphan_file() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "foo", "function", 1);
        relations.add_symbol("b.py", "bar", "function", 1);
        relations.add_symbol("c.py", "baz", "function", 1);
        relations.add_import("a.py", "b.py", "bar");
        // c.py is never imported

        // Orphan-file is disabled by default, force-enable for test
        let mut rule = find_builtin("orphan-file");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        let messages: Vec<&str> = result.iter().map(|d| d.message.as_str()).collect();
        assert!(messages.contains(&"a.py")); // a.py is also an orphan (not imported)
        assert!(messages.contains(&"c.py"));
        assert!(!messages.contains(&"b.py")); // b.py is imported
    }

    #[test]
    fn test_self_import() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "a.py", "foo");
        relations.add_import("b.py", "c.py", "bar");

        let rule = find_builtin("self-import");
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "a.py");
    }

    #[test]
    fn test_god_file() {
        let mut relations = Relations::new();
        // Add 51 symbols to big.py
        for i in 0..51 {
            relations.add_symbol("big.py", &format!("sym_{}", i), "function", i);
        }
        // Add 5 symbols to small.py
        for i in 0..5 {
            relations.add_symbol("small.py", &format!("sym_{}", i), "function", i);
        }

        let rule = find_builtin("god-file");
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "big.py");
    }

    #[test]
    fn test_fan_out() {
        let mut relations = Relations::new();
        // Add 51 calls from the same function (threshold is >50)
        for i in 0..51 {
            relations.add_call("a.py", "orchestrator", &format!("helper_{}", i), i);
        }
        // Add 3 calls from a simple function
        for i in 0..3 {
            relations.add_call("b.py", "simple", &format!("util_{}", i), i);
        }

        // Fan-out is disabled by default, force-enable for test
        let mut rule = find_builtin("fan-out");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "orchestrator");
    }

    #[test]
    fn test_hub_file() {
        let mut relations = Relations::new();
        // 31 files import utils.py (threshold is >30)
        for i in 0..31 {
            relations.add_import(&format!("file_{}.py", i), "utils.py", "helper");
        }
        // Only 2 files import rare.py
        relations.add_import("a.py", "rare.py", "x");
        relations.add_import("b.py", "rare.py", "y");

        // Hub-file is disabled by default, force-enable for test
        let mut rule = find_builtin("hub-file");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "utils.py");
    }

    #[test]
    fn test_duplicate_symbol_disabled_by_default() {
        // duplicate-symbol is disabled by default in frontmatter
        let builtin = BUILTIN_RULES
            .iter()
            .find(|b| b.id == "duplicate-symbol")
            .unwrap();
        let rule = parse_rule_content(builtin.content, builtin.id, true).unwrap();
        assert!(!rule.enabled);
    }

    #[test]
    fn test_duplicate_symbol_when_enabled() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "process", "function", 1);
        relations.add_symbol("b.py", "process", "function", 5);
        relations.add_symbol("c.py", "unique", "function", 1);

        // Parse and force-enable
        let builtin = BUILTIN_RULES
            .iter()
            .find(|b| b.id == "duplicate-symbol")
            .unwrap();
        let mut rule = parse_rule_content(builtin.content, builtin.id, true).unwrap();
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "process");
    }

    /// Helper to find and parse a builtin rule by ID.
    fn find_builtin(id: &str) -> FactsRule {
        let builtin = BUILTIN_RULES.iter().find(|b| b.id == id).unwrap();
        parse_rule_content(builtin.content, builtin.id, true).unwrap()
    }
}
