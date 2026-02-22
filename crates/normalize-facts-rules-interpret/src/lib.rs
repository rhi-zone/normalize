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
//! - `visibility(file: String, name: String, vis: String)` — "public", "private", "protected", "internal"
//! - `attribute(file: String, name: String, attr: String)` — one row per attribute per symbol
//! - `parent(file: String, child_name: String, parent_name: String)` — symbol nesting hierarchy
//! - `qualifier(caller_file: String, caller_name: String, callee_name: String, qual: String)` — call qualifier
//! - `symbol_range(file: String, name: String, start_line: u32, end_line: u32)` — symbol span
//! - `implements(file: String, name: String, interface: String)` — interface/trait implementation
//! - `is_impl(file: String, name: String)` — symbol is a trait/interface implementation
//! - `type_method(file: String, type_name: String, method_name: String)` — method signatures on types
//!
//! Output relations are read as diagnostics:
//! - `warning(rule_id: String, message: String)` → produces warnings
//! - `error(rule_id: String, message: String)` → produces errors

use ascent_eval::{Engine, Value};
use ascent_ir::Program;
use ascent_syntax::AscentProgram;
use glob::Pattern;
use normalize_derive::Merge;
use normalize_facts_rules_api::{Diagnostic, DiagnosticLevel, Relations};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Preamble declaring the built-in input relations.
/// Users don't need to declare these - they're always available.
const PREAMBLE: &str = r#"
relation symbol(String, String, String, u32);
relation import(String, String, String);
relation call(String, String, String, u32);
relation visibility(String, String, String);
relation attribute(String, String, String);
relation parent(String, String, String);
relation qualifier(String, String, String, String);
relation symbol_range(String, String, u32, u32);
relation implements(String, String, String);
relation is_impl(String, String);
relation type_method(String, String, String);
relation warning(String, String);
relation error(String, String);
"#;

// =============================================================================
// Configuration
// =============================================================================

/// Per-rule configuration override from config.toml.
///
/// ```toml
/// [facts-rules."god-file"]
/// deny = true
/// allow = ["src/generated/**"]
///
/// [facts-rules."fan-out"]
/// enabled = true
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default, schemars::JsonSchema)]
#[serde(default)]
pub struct FactsRuleOverride {
    /// Promote all warnings from this rule to errors (deprecated: use severity).
    pub deny: Option<bool>,
    /// Override the rule's severity (error, warning, info).
    pub severity: Option<String>,
    /// Enable or disable the rule.
    pub enabled: Option<bool>,
    /// Additional patterns to allow (suppress matching diagnostics).
    #[serde(default)]
    pub allow: Vec<String>,
    /// Additional tags to add to this rule (appends to built-in tags).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Configuration for fact rules. Maps rule ID to per-rule overrides.
#[derive(Debug, Clone, Deserialize, Serialize, Default, Merge, schemars::JsonSchema)]
#[serde(transparent)]
pub struct FactsRulesConfig(pub HashMap<String, FactsRuleOverride>);

// =============================================================================
// Rule types and loading
// =============================================================================

/// Severity level for rule findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    Error,
    #[default]
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Severity::Error),
            "warning" | "warn" => Ok(Severity::Warning),
            "info" | "note" => Ok(Severity::Info),
            _ => Err(format!("unknown severity: {}", s)),
        }
    }
}

/// A Datalog fact rule definition.
#[derive(Debug)]
pub struct FactsRule {
    /// Unique identifier for this rule.
    pub id: String,
    /// The Datalog source (without frontmatter).
    pub source: String,
    /// Description for display when listing rules.
    pub message: String,
    /// Glob patterns for diagnostic messages to suppress.
    pub allow: Vec<Pattern>,
    /// Severity level for diagnostics from this rule.
    pub severity: Severity,
    /// Whether this rule is enabled.
    pub enabled: bool,
    /// Whether this is a builtin rule.
    pub builtin: bool,
    /// Source file path (empty for builtins).
    pub source_path: PathBuf,
    /// Tags for grouping and filtering rules by concept (e.g. "architecture", "complexity").
    pub tags: Vec<String>,
    /// Documentation from the markdown comment block between frontmatter and source.
    pub doc: Option<String>,
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
    BuiltinFactsRule {
        id: "god-class",
        content: include_str!("builtin_dl/god_class.dl"),
    },
    BuiltinFactsRule {
        id: "long-function",
        content: include_str!("builtin_dl/long_function.dl"),
    },
    BuiltinFactsRule {
        id: "dead-api",
        content: include_str!("builtin_dl/dead_api.dl"),
    },
    BuiltinFactsRule {
        id: "missing-impl",
        content: include_str!("builtin_dl/missing_impl.dl"),
    },
    BuiltinFactsRule {
        id: "unused-import",
        content: include_str!("builtin_dl/unused_import.dl"),
    },
    BuiltinFactsRule {
        id: "barrel-file",
        content: include_str!("builtin_dl/barrel_file.dl"),
    },
    BuiltinFactsRule {
        id: "bidirectional-deps",
        content: include_str!("builtin_dl/bidirectional_deps.dl"),
    },
    BuiltinFactsRule {
        id: "deep-nesting",
        content: include_str!("builtin_dl/deep_nesting.dl"),
    },
    BuiltinFactsRule {
        id: "layering-violation",
        content: include_str!("builtin_dl/layering_violation.dl"),
    },
    BuiltinFactsRule {
        id: "missing-export",
        content: include_str!("builtin_dl/missing_export.dl"),
    },
];

/// Load all rules from all sources, merged by ID.
/// Order: builtins → ~/.config/normalize/rules/ → .normalize/rules/
/// Then applies config overrides (deny, enabled, allow).
pub fn load_all_rules(project_root: &Path, config: &FactsRulesConfig) -> Vec<FactsRule> {
    let mut rules_by_id: HashMap<String, FactsRule> = HashMap::new();

    // 1. Load embedded builtins
    for builtin in BUILTIN_RULES {
        if let Some(rule) = parse_rule_content(builtin.content, builtin.id, true) {
            rules_by_id.insert(rule.id.clone(), rule);
        }
    }

    // 2. Load user global rules (~/.config/normalize/rules/)
    if let Some(config_dir) = dirs::config_dir() {
        let user_rules_dir = config_dir.join("normalize").join("rules");
        for rule in load_rules_from_dir(&user_rules_dir) {
            rules_by_id.insert(rule.id.clone(), rule);
        }
    }

    // 3. Load project rules (.normalize/rules/)
    let project_rules_dir = project_root.join(".normalize").join("rules");
    for rule in load_rules_from_dir(&project_rules_dir) {
        rules_by_id.insert(rule.id.clone(), rule);
    }

    // 4. Apply config overrides
    for (rule_id, override_cfg) in &config.0 {
        if let Some(rule) = rules_by_id.get_mut(rule_id) {
            // severity takes precedence over deny
            if let Some(ref sev_str) = override_cfg.severity {
                if let Ok(sev) = sev_str.parse::<Severity>() {
                    rule.severity = sev;
                }
            } else if override_cfg.deny == Some(true) {
                rule.severity = Severity::Error;
            }
            if let Some(enabled) = override_cfg.enabled {
                rule.enabled = enabled;
            }
            for pattern_str in &override_cfg.allow {
                if let Ok(pattern) = Pattern::new(pattern_str) {
                    rule.allow.push(pattern);
                }
            }
            for tag in &override_cfg.tags {
                if !rule.tags.contains(tag) {
                    rule.tags.push(tag.clone());
                }
            }
        }
    }

    rules_by_id.into_values().collect()
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
/// # allow = ["**/tests/**"]
/// # enabled = true
/// # ---
/// ```
pub fn parse_rule_content(content: &str, default_id: &str, is_builtin: bool) -> Option<FactsRule> {
    let lines: Vec<&str> = content.lines().collect();

    let mut in_frontmatter = false;
    let mut frontmatter_done = false;
    let mut frontmatter_lines = Vec::new();
    let mut doc_lines = Vec::new();
    let mut source_lines = Vec::new();

    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "# ---" {
            if in_frontmatter {
                frontmatter_done = true;
            }
            in_frontmatter = !in_frontmatter;
            continue;
        }

        if in_frontmatter {
            let fm_line = line.strip_prefix('#').unwrap_or(line).trim_start();
            frontmatter_lines.push(fm_line);
        } else if frontmatter_done && source_lines.is_empty() && trimmed.starts_with('#') {
            // Doc block: comment lines after frontmatter, before source
            let doc_line = line.strip_prefix('#').unwrap_or("").trim_start_matches(' ');
            doc_lines.push(doc_line);
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

    let doc = if doc_lines.is_empty() {
        None
    } else {
        let text = doc_lines.join("\n").trim().to_string();
        if text.is_empty() { None } else { Some(text) }
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

    let allow: Vec<Pattern> = frontmatter
        .get("allow")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .filter_map(|s| Pattern::new(s).ok())
                .collect()
        })
        .unwrap_or_default();

    let severity = if let Some(sev_str) = frontmatter.get("severity").and_then(|v| v.as_str()) {
        sev_str.parse::<Severity>().unwrap_or(Severity::Warning)
    } else if frontmatter
        .get("deny")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        Severity::Error
    } else {
        Severity::Warning
    };

    let enabled = frontmatter
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let tags: Vec<String> = frontmatter
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    Some(FactsRule {
        id,
        source: source_str.trim().to_string(),
        message,
        allow,
        severity,
        enabled,
        builtin: is_builtin,
        source_path: PathBuf::new(),
        tags,
        doc,
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
/// Filters diagnostics through `allow` patterns and applies `deny` promotion.
pub fn run_rule(
    rule: &FactsRule,
    relations: &Relations,
) -> Result<Vec<Diagnostic>, InterpretError> {
    let mut diagnostics = run_rules_source(&rule.source, relations)?;

    // Filter out allowed diagnostics
    if !rule.allow.is_empty() {
        diagnostics.retain(|d| !rule.allow.iter().any(|p| p.matches(d.message.as_str())));
    }

    // Apply severity: promote or demote diagnostics
    match rule.severity {
        Severity::Error => {
            for d in &mut diagnostics {
                d.level = DiagnosticLevel::Error;
            }
        }
        Severity::Info => {
            for d in &mut diagnostics {
                if d.level == DiagnosticLevel::Warning {
                    d.level = DiagnosticLevel::Hint;
                }
            }
        }
        Severity::Warning => {} // default, no change
    }

    Ok(diagnostics)
}

/// Filter out diagnostics suppressed by `normalize-facts-allow: rule-id` comments in source files.
///
/// When a diagnostic's message is a file path (relative to `root`), the first 10
/// lines of that file are checked for `// normalize-facts-allow: rule-id` or
/// `# normalize-facts-allow: rule-id`. This mirrors the inline suppression mechanism
/// from syntax-rules.
pub fn filter_inline_allowed(diagnostics: &mut Vec<Diagnostic>, root: &Path) {
    diagnostics.retain(|d| {
        let path = root.join(d.message.as_str());
        if path.is_file() {
            !file_has_allow_comment(&path, d.rule_id.as_str())
        } else {
            true // not a file path, keep it
        }
    });
}

/// Check if a file's header contains a `normalize-facts-allow: rule-id` comment.
fn file_has_allow_comment(path: &Path, rule_id: &str) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    content
        .lines()
        .take(10)
        .any(|line| line_has_allow_comment(line, rule_id))
}

/// Check if a line contains a `normalize-facts-allow: rule-id` comment.
/// Supports `// normalize-facts-allow: rule-id`, `# normalize-facts-allow: rule-id`,
/// `/* normalize-facts-allow: rule-id */`, with optional `- reason` suffix.
fn line_has_allow_comment(line: &str, rule_id: &str) -> bool {
    if let Some(pos) = line.find("normalize-facts-allow:") {
        let after = &line[pos + 22..]; // len("normalize-facts-allow:")
        let after = after.trim_start();
        if let Some(rest) = after.strip_prefix(rule_id) {
            return rest.is_empty()
                || rest.starts_with(char::is_whitespace)
                || rest.starts_with('-')
                || rest.starts_with("*/");
        }
    }
    false
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

    for vis in relations.visibilities.iter() {
        engine.insert(
            "visibility",
            vec![
                Value::String(Rc::new(vis.file.to_string())),
                Value::String(Rc::new(vis.name.to_string())),
                Value::String(Rc::new(vis.visibility.to_string())),
            ],
        );
    }

    for attr in relations.attributes.iter() {
        engine.insert(
            "attribute",
            vec![
                Value::String(Rc::new(attr.file.to_string())),
                Value::String(Rc::new(attr.name.to_string())),
                Value::String(Rc::new(attr.attribute.to_string())),
            ],
        );
    }

    for p in relations.parents.iter() {
        engine.insert(
            "parent",
            vec![
                Value::String(Rc::new(p.file.to_string())),
                Value::String(Rc::new(p.child_name.to_string())),
                Value::String(Rc::new(p.parent_name.to_string())),
            ],
        );
    }

    for q in relations.qualifiers.iter() {
        engine.insert(
            "qualifier",
            vec![
                Value::String(Rc::new(q.caller_file.to_string())),
                Value::String(Rc::new(q.caller_name.to_string())),
                Value::String(Rc::new(q.callee_name.to_string())),
                Value::String(Rc::new(q.qualifier.to_string())),
            ],
        );
    }

    for sr in relations.symbol_ranges.iter() {
        engine.insert(
            "symbol_range",
            vec![
                Value::String(Rc::new(sr.file.to_string())),
                Value::String(Rc::new(sr.name.to_string())),
                Value::U32(sr.start_line),
                Value::U32(sr.end_line),
            ],
        );
    }

    for imp in relations.implements.iter() {
        engine.insert(
            "implements",
            vec![
                Value::String(Rc::new(imp.file.to_string())),
                Value::String(Rc::new(imp.name.to_string())),
                Value::String(Rc::new(imp.interface.to_string())),
            ],
        );
    }

    for ii in relations.is_impls.iter() {
        engine.insert(
            "is_impl",
            vec![
                Value::String(Rc::new(ii.file.to_string())),
                Value::String(Rc::new(ii.name.to_string())),
            ],
        );
    }

    for tm in relations.type_methods.iter() {
        engine.insert(
            "type_method",
            vec![
                Value::String(Rc::new(tm.file.to_string())),
                Value::String(Rc::new(tm.type_name.to_string())),
                Value::String(Rc::new(tm.method_name.to_string())),
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
    fn test_allow_patterns() {
        let content = r#"
# ---
# id = "test-allow"
# allow = ["**/tests/**", "**/*_test.py"]
# ---

warning("test-allow", file) <-- symbol(file, _, _, _);
"#;

        let mut relations = Relations::new();
        relations.add_symbol("src/main.py", "foo", "function", 1);
        relations.add_symbol("tests/test_foo.py", "test_foo", "function", 1);
        relations.add_symbol("src/foo_test.py", "bar", "function", 1);

        let rule = parse_rule_content(content, "test-allow", false).unwrap();
        assert_eq!(rule.allow.len(), 2);

        let result = run_rule(&rule, &relations).unwrap();
        // tests/test_foo.py matches **/tests/**, foo_test.py matches **/*_test.py
        // Only src/main.py should remain
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "src/main.py");
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

    #[test]
    fn test_deny_promotes_warnings_to_errors() {
        let content = r#"
# ---
# id = "strict-rule"
# deny = true
# ---

warning("strict-rule", file) <-- symbol(file, _, _, _);
"#;

        let mut relations = Relations::new();
        relations.add_symbol("a.py", "foo", "function", 1);

        let rule = parse_rule_content(content, "strict-rule", false).unwrap();
        assert_eq!(rule.severity, Severity::Error);

        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].level, DiagnosticLevel::Error); // promoted from warning
    }

    #[test]
    fn test_config_override_deny() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "foo", "function", 1);

        // self-import has severity=warning by default
        let rule = find_builtin("self-import");
        assert_eq!(rule.severity, Severity::Warning);

        // Apply config override with deny=true (legacy)
        let mut config = FactsRulesConfig::default();
        config.0.insert(
            "self-import".to_string(),
            FactsRuleOverride {
                deny: Some(true),
                ..Default::default()
            },
        );

        // Load with config
        let rules = load_all_rules(Path::new("/nonexistent"), &config);
        let self_import = rules.iter().find(|r| r.id == "self-import").unwrap();
        assert_eq!(self_import.severity, Severity::Error);
    }

    #[test]
    fn test_config_override_allow() {
        let mut relations = Relations::new();
        for i in 0..51 {
            relations.add_symbol("big.py", &format!("sym_{}", i), "function", i);
        }
        for i in 0..51 {
            relations.add_symbol("generated/big.py", &format!("sym_{}", i), "function", i);
        }

        // Without config override, both files trigger god-file
        let mut rule = find_builtin("god-file");
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 2);

        // With config override, suppress generated/ files
        rule.allow.push(Pattern::new("generated/**").unwrap());
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "big.py");
    }

    #[test]
    fn test_config_override_enable() {
        // fan-out is disabled by default
        let default_config = FactsRulesConfig::default();
        let rules = load_all_rules(Path::new("/nonexistent"), &default_config);
        let fan_out = rules.iter().find(|r| r.id == "fan-out").unwrap();
        assert!(!fan_out.enabled);

        // Enable via config
        let mut config = FactsRulesConfig::default();
        config.0.insert(
            "fan-out".to_string(),
            FactsRuleOverride {
                enabled: Some(true),
                ..Default::default()
            },
        );
        let rules = load_all_rules(Path::new("/nonexistent"), &config);
        let fan_out = rules.iter().find(|r| r.id == "fan-out").unwrap();
        assert!(fan_out.enabled);
    }

    #[test]
    fn test_line_has_allow_comment() {
        assert!(line_has_allow_comment(
            "// normalize-facts-allow: god-file",
            "god-file"
        ));
        assert!(line_has_allow_comment(
            "# normalize-facts-allow: god-file",
            "god-file"
        ));
        assert!(line_has_allow_comment(
            "/* normalize-facts-allow: god-file */",
            "god-file"
        ));
        assert!(line_has_allow_comment(
            "// normalize-facts-allow: god-file - this file is intentionally large",
            "god-file"
        ));
        assert!(!line_has_allow_comment(
            "// normalize-facts-allow: god-file",
            "fan-out"
        ));
        assert!(!line_has_allow_comment(
            "// no suppression here",
            "god-file"
        ));
    }

    #[test]
    fn test_filter_inline_allowed() {
        let dir = std::env::temp_dir().join("normalize_test_inline_allow");
        let _ = std::fs::create_dir_all(&dir);

        // File with suppression comment
        std::fs::write(
            dir.join("suppressed.py"),
            "# normalize-facts-allow: test-rule\ndef foo(): pass\n",
        )
        .unwrap();

        // File without suppression
        std::fs::write(dir.join("normal.py"), "def bar(): pass\n").unwrap();

        let mut diagnostics = vec![
            Diagnostic::warning("test-rule", "suppressed.py"),
            Diagnostic::warning("test-rule", "normal.py"),
            Diagnostic::warning("test-rule", "nonexistent.py"), // not a file, kept
        ];

        filter_inline_allowed(&mut diagnostics, &dir);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].message.as_str(), "normal.py");
        assert_eq!(diagnostics[1].message.as_str(), "nonexistent.py");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_visibility_relation() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "foo", "function", 1);
        relations.add_symbol("a.py", "_bar", "function", 5);
        relations.add_visibility("a.py", "foo", "public");
        relations.add_visibility("a.py", "_bar", "private");

        let rules = r#"
            relation priv_func(String, String);
            priv_func(file, name) <-- visibility(file, name, vis), if vis == "private";
            warning("private-func", name) <-- priv_func(_, name);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "_bar");
    }

    #[test]
    fn test_attribute_relation() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "foo", "function", 1);
        relations.add_attribute("a.py", "foo", "@staticmethod");
        relations.add_attribute("a.py", "foo", "@override");

        let rules = r##"
            relation static_fn(String, String);
            static_fn(file, name) <-- attribute(file, name, attr), if attr == "@staticmethod";
            warning("static-fn", name) <-- static_fn(_, name);
        "##;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "foo");
    }

    #[test]
    fn test_parent_relation() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "MyClass", "class", 1);
        relations.add_symbol("a.py", "method_a", "method", 2);
        relations.add_symbol("a.py", "method_b", "method", 5);
        relations.add_parent("a.py", "method_a", "MyClass");
        relations.add_parent("a.py", "method_b", "MyClass");

        let rules = r#"
            relation method_count(String, String, i32);
            method_count(file, cls, c) <--
                parent(file, _, cls),
                agg c = count() in parent(file, _, cls);
            warning("big-class", cls) <-- method_count(_, cls, c), if c > 1;
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "MyClass");
    }

    #[test]
    fn test_qualifier_relation() {
        let mut relations = Relations::new();
        relations.add_call("a.py", "method_a", "method_b", 3);
        relations.add_qualifier("a.py", "method_a", "method_b", "self");
        relations.add_call("a.py", "main", "helper", 10);

        let rules = r#"
            relation self_call(String, String, String);
            self_call(file, caller, callee) <-- qualifier(file, caller, callee, q), if q == "self";
            warning("self-call", callee) <-- self_call(_, _, callee);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "method_b");
    }

    #[test]
    fn test_symbol_range_relation() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "big_func", "function", 1);
        relations.add_symbol("a.py", "small_func", "function", 50);
        relations.add_symbol_range("a.py", "big_func", 1, 100);
        relations.add_symbol_range("a.py", "small_func", 50, 55);

        let rules = r#"
            relation long_fn(String, String, u32);
            long_fn(file, name, len) <--
                symbol_range(file, name, start, end),
                let len = end - start,
                if len > 20u32;
            warning("long-fn", name) <-- long_fn(_, name, _);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "big_func");
    }

    #[test]
    fn test_implements_relation() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "MyClass", "class", 1);
        relations.add_implements("a.py", "MyClass", "Serializable");
        relations.add_implements("a.py", "MyClass", "Comparable");
        relations.add_symbol("b.py", "OtherClass", "class", 1);

        let rules = r#"
            relation impl_count(String, String, i32);
            impl_count(file, name, c) <--
                implements(file, name, _),
                agg c = count() in implements(file, name, _);
            warning("multi-impl", name) <-- impl_count(_, name, c), if c > 1;
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "MyClass");
    }

    #[test]
    fn test_is_impl_relation() {
        let mut relations = Relations::new();
        relations.add_symbol("a.rs", "impl_method", "method", 5);
        relations.add_symbol("a.rs", "free_func", "function", 20);
        relations.add_is_impl("a.rs", "impl_method");

        let rules = r#"
            warning("is-impl", name) <-- is_impl(_, name);
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "impl_method");
    }

    #[test]
    fn test_type_method_relation() {
        let mut relations = Relations::new();
        relations.add_type_method("a.py", "Animal", "speak");
        relations.add_type_method("a.py", "Animal", "move");
        relations.add_type_method("b.py", "Vehicle", "drive");

        let rules = r#"
            relation method_count(String, String, i32);
            method_count(file, t, c) <--
                type_method(file, t, _),
                agg c = count() in type_method(file, t, _);
            warning("rich-type", t) <-- method_count(_, t, c), if c > 1;
        "#;

        let result = run_rules_source(rules, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "Animal");
    }

    #[test]
    fn test_god_class_fires() {
        let mut relations = Relations::new();
        // Add a class with 21 methods (threshold is >20)
        relations.add_symbol("a.py", "BigClass", "class", 1);
        for i in 0..21 {
            let method = format!("method_{}", i);
            relations.add_symbol("a.py", &method, "method", i + 2);
            relations.add_parent("a.py", &method, "BigClass");
        }
        // Add a small class (should not fire)
        relations.add_symbol("a.py", "SmallClass", "class", 100);
        relations.add_symbol("a.py", "do_thing", "method", 101);
        relations.add_parent("a.py", "do_thing", "SmallClass");

        let mut rule = find_builtin("god-class");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "BigClass");
    }

    #[test]
    fn test_god_class_no_fire() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "NormalClass", "class", 1);
        for i in 0..5 {
            let method = format!("method_{}", i);
            relations.add_symbol("a.py", &method, "method", i + 2);
            relations.add_parent("a.py", &method, "NormalClass");
        }

        let mut rule = find_builtin("god-class");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_long_function_fires() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "huge_func", "function", 1);
        relations.add_symbol_range("a.py", "huge_func", 1, 150);
        relations.add_symbol("a.py", "tiny_func", "function", 200);
        relations.add_symbol_range("a.py", "tiny_func", 200, 210);

        let mut rule = find_builtin("long-function");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "huge_func");
    }

    #[test]
    fn test_long_function_no_fire() {
        let mut relations = Relations::new();
        relations.add_symbol("a.py", "short_func", "function", 1);
        relations.add_symbol_range("a.py", "short_func", 1, 50);

        let mut rule = find_builtin("long-function");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_dead_api_fires() {
        let mut relations = Relations::new();
        // Public function defined in a.py, never called from another file
        relations.add_symbol("a.py", "unused_pub", "function", 1);
        relations.add_visibility("a.py", "unused_pub", "public");
        // Public function defined in b.py, called from a.py (not dead)
        relations.add_symbol("b.py", "used_pub", "function", 1);
        relations.add_visibility("b.py", "used_pub", "public");
        relations.add_call("a.py", "main", "used_pub", 5);

        let mut rule = find_builtin("dead-api");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "unused_pub");
    }

    #[test]
    fn test_dead_api_no_fire() {
        let mut relations = Relations::new();
        // Public function called from another file
        relations.add_symbol("a.py", "helper", "function", 1);
        relations.add_visibility("a.py", "helper", "public");
        relations.add_call("b.py", "main", "helper", 5);

        let mut rule = find_builtin("dead-api");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_missing_impl_fires() {
        let mut relations = Relations::new();
        // Interface with 2 methods
        relations.add_type_method("iface.ts", "Serializable", "serialize");
        relations.add_type_method("iface.ts", "Serializable", "deserialize");
        // Class implements Serializable but only has serialize
        relations.add_symbol("impl.ts", "MyClass", "class", 1);
        relations.add_implements("impl.ts", "MyClass", "Serializable");
        relations.add_parent("impl.ts", "serialize", "MyClass");
        // Missing: deserialize

        let mut rule = find_builtin("missing-impl");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "MyClass");
    }

    #[test]
    fn test_missing_impl_no_fire() {
        let mut relations = Relations::new();
        // Interface with 1 method
        relations.add_type_method("iface.ts", "Runnable", "run");
        // Class implements Runnable and has the method
        relations.add_symbol("impl.ts", "Worker", "class", 1);
        relations.add_implements("impl.ts", "Worker", "Runnable");
        relations.add_parent("impl.ts", "run", "Worker");

        let mut rule = find_builtin("missing-impl");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_unused_import_fires() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "helper");
        // "helper" is never called in a.py

        let mut rule = find_builtin("unused-import");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "helper");
    }

    #[test]
    fn test_unused_import_no_fire() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "helper");
        relations.add_call("a.py", "main", "helper", 5);

        let mut rule = find_builtin("unused-import");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_unused_import_wildcard_ignored() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "*");

        let mut rule = find_builtin("unused-import");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_barrel_file_fires() {
        let mut relations = Relations::new();
        // File with only imports, no own symbols
        relations.add_import("index.ts", "a.ts", "foo");
        relations.add_import("index.ts", "b.ts", "bar");

        let mut rule = find_builtin("barrel-file");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "index.ts");
    }

    #[test]
    fn test_barrel_file_no_fire() {
        let mut relations = Relations::new();
        relations.add_import("app.ts", "utils.ts", "helper");
        relations.add_symbol("app.ts", "main", "function", 1);

        let mut rule = find_builtin("barrel-file");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_bidirectional_deps_fires() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "foo");
        relations.add_import("b.py", "a.py", "bar");

        let mut rule = find_builtin("bidirectional-deps");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_bidirectional_deps_no_fire() {
        let mut relations = Relations::new();
        relations.add_import("a.py", "b.py", "foo");
        relations.add_import("a.py", "c.py", "bar");

        let mut rule = find_builtin("bidirectional-deps");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_deep_nesting_fires() {
        let mut relations = Relations::new();
        // 5 levels: Top -> A -> B -> C -> TooDeep (4 parent hops = >3)
        relations.add_symbol("a.py", "Top", "class", 1);
        relations.add_symbol("a.py", "A", "class", 5);
        relations.add_parent("a.py", "A", "Top");
        relations.add_symbol("a.py", "B", "class", 10);
        relations.add_parent("a.py", "B", "A");
        relations.add_symbol("a.py", "C", "class", 15);
        relations.add_parent("a.py", "C", "B");
        relations.add_symbol("a.py", "TooDeep", "function", 20);
        relations.add_parent("a.py", "TooDeep", "C");

        let mut rule = find_builtin("deep-nesting");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        let messages: Vec<&str> = result.iter().map(|d| d.message.as_str()).collect();
        assert!(messages.contains(&"TooDeep"));
    }

    #[test]
    fn test_deep_nesting_no_fire() {
        let mut relations = Relations::new();
        // 2 levels: MyClass -> method (only 1 parent hop)
        relations.add_symbol("a.py", "MyClass", "class", 1);
        relations.add_symbol("a.py", "method", "method", 5);
        relations.add_parent("a.py", "method", "MyClass");

        let mut rule = find_builtin("deep-nesting");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_layering_violation_fires() {
        let mut relations = Relations::new();
        // Both files have test attributes
        relations.add_symbol("test_a.py", "test_foo", "function", 1);
        relations.add_attribute("test_a.py", "test_foo", "#[test]");
        relations.add_symbol("test_b.py", "test_bar", "function", 1);
        relations.add_attribute("test_b.py", "test_bar", "#[test]");
        relations.add_import("test_a.py", "test_b.py", "helper");

        let mut rule = find_builtin("layering-violation");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "test_a.py");
    }

    #[test]
    fn test_layering_violation_no_fire() {
        let mut relations = Relations::new();
        // test_a has test attribute, utils does not
        relations.add_symbol("test_a.py", "test_foo", "function", 1);
        relations.add_attribute("test_a.py", "test_foo", "#[test]");
        relations.add_symbol("utils.py", "helper", "function", 1);
        relations.add_import("test_a.py", "utils.py", "helper");

        let mut rule = find_builtin("layering-violation");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_missing_export_fires() {
        let mut relations = Relations::new();
        relations.add_symbol("utils.py", "helper", "function", 1);
        relations.add_visibility("utils.py", "helper", "public");
        // No file imports utils.py

        let mut rule = find_builtin("missing-export");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.as_str(), "utils.py");
    }

    #[test]
    fn test_missing_export_no_fire() {
        let mut relations = Relations::new();
        relations.add_symbol("utils.py", "helper", "function", 1);
        relations.add_visibility("utils.py", "helper", "public");
        relations.add_import("app.py", "utils.py", "helper");

        let mut rule = find_builtin("missing-export");
        rule.enabled = true;
        let result = run_rule(&rule, &relations).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_new_builtins_parse() {
        // These rules should be disabled by default
        for id in &["barrel-file", "layering-violation", "missing-export"] {
            let rule = find_builtin(id);
            assert!(!rule.enabled, "{} should be disabled by default", id);
            assert!(rule.builtin, "{} should be builtin", id);
        }

        // These rules should be enabled by default
        for id in &[
            "unused-import",
            "bidirectional-deps",
            "deep-nesting",
            "god-class",
            "long-function",
        ] {
            let rule = find_builtin(id);
            assert!(rule.enabled, "{} should be enabled by default", id);
            assert!(rule.builtin, "{} should be builtin", id);
        }
    }

    /// Helper to find and parse a builtin rule by ID.
    fn find_builtin(id: &str) -> FactsRule {
        let builtin = BUILTIN_RULES.iter().find(|b| b.id == id).unwrap();
        parse_rule_content(builtin.content, builtin.id, true).unwrap()
    }
}
