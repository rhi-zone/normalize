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
const BUILTIN_RULES: &[BuiltinFactsRule] = &[BuiltinFactsRule {
    id: "circular-deps",
    content: include_str!("builtin_dl/circular_deps.dl"),
}];

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
            assert!(rule.enabled);
            assert!(!rule.source.is_empty());
        }
    }

    #[test]
    fn test_run_builtin_cycle_detection() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "*");
        relations.add_import("b.py", "a.py", "*");

        let builtin = &BUILTIN_RULES[0]; // circular-deps
        let rule = parse_rule_content(builtin.content, builtin.id, true).unwrap();
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].rule_id.as_str(), "circular-deps");
    }
}
