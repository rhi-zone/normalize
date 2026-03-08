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
//! Output relation — all diagnostics go here:
//! - `diagnostic(severity, rule_id, file, line, message)` — severity = "warning"/"error"/"info"/"hint";
//!   file = "" for no location; line = 0 when the source has no line info.

use abi_stable::std_types::ROption;
use ascent_eval::{Engine, Value};
use ascent_ir::Program;
use ascent_syntax::AscentProgram;
use glob::Pattern;
use normalize_core::Merge;
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
relation diagnostic(String, String, String, u32, String);
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

    // Filter out allowed diagnostics.
    // For located diagnostics (file != ""), match the allow glob against the file path.
    // For unlocated diagnostics, match against the message (e.g. hub-file puts module
    // name in message).
    if !rule.allow.is_empty() {
        diagnostics.retain(|d| {
            let match_str = match d.location.as_ref() {
                ROption::RSome(loc) => loc.file.as_str(),
                ROption::RNone => d.message.as_str(),
            };
            !rule.allow.iter().any(|p| p.matches(match_str))
        });
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
        // For file-located diagnostics, check the location file directly.
        // For unlocated diagnostics, try interpreting the message as a file path.
        let file_str = match d.location.as_ref() {
            ROption::RSome(loc) => loc.file.as_str(),
            ROption::RNone => d.message.as_str(),
        };
        let path = root.join(file_str);
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

/// Extract diagnostics from the `diagnostic` output relation.
///
/// `diagnostic(severity, rule_id, file, line, message)`:
/// - severity: "error", "warning", "info", "hint"
/// - file: "" for no specific location
/// - line: 0 when the fact source has no line info
fn extract_diagnostics(engine: &Engine) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if let Some(diags) = engine.relation("diagnostic") {
        for tuple in diags.iter() {
            if let [
                Value::String(severity),
                Value::String(rule_id),
                Value::String(file),
                Value::U32(line),
                Value::String(message),
            ] = tuple
            {
                let mut d = match severity.as_str() {
                    "error" => Diagnostic::error(rule_id, message),
                    "info" | "hint" => Diagnostic::hint(rule_id, message),
                    _ => Diagnostic::warning(rule_id, message),
                };
                if !file.is_empty() {
                    d = d.at(file, *line);
                }
                diagnostics.push(d);
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests;
