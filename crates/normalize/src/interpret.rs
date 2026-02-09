//! Interpreted Datalog rule evaluation using ascent-interpreter.
//!
//! This module bridges normalize's Relations to the ascent-interpreter engine,
//! enabling users to write `.dl` files that run directly without compilation.
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
use std::path::Path;
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
    // Populate symbol facts
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

    // Populate import facts
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

    // Populate call facts
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

    // Read warnings
    if let Some(warnings) = engine.relation("warning") {
        for tuple in warnings.iter() {
            if let [Value::String(rule_id), Value::String(message)] = tuple {
                diagnostics.push(Diagnostic::warning(rule_id, message));
            }
        }
    }

    // Read errors
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
}
