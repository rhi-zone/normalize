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
use ascent_interpreter::eval::{Engine, SharedJitCompiler, SourceId, Value};
use ascent_interpreter::ir::Program;
use ascent_interpreter::syntax::AscentProgram;
use glob::Pattern;
use normalize_facts_rules_api::{Diagnostic, DiagnosticLevel, Relations};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

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

pub use normalize_rules_config::RuleOverride;
pub use normalize_rules_config::RulesConfig;

// =============================================================================
// Rule types and loading
// =============================================================================

/// Severity level for rule findings. Defined in normalize-rules-config for sharing
/// across all rule engines (syntax, fact).
pub use normalize_rules_config::Severity;

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
    /// Whether this rule is recommended for most projects (catches real bugs, not style).
    pub recommended: bool,
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
pub fn load_all_rules(project_root: &Path, config: &RulesConfig) -> Vec<FactsRule> {
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
    for (rule_id, override_cfg) in &config.rules {
        if let Some(rule) = rules_by_id.get_mut(rule_id) {
            if let Some(ref sev_str) = override_cfg.severity
                && let Ok(sev) = sev_str.parse::<Severity>()
            {
                rule.severity = sev;
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
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("failed to read rule file {:?}: {}", path, e);
            return None;
        }
    };
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
                tracing::warn!("invalid frontmatter: {}", e);
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

    let recommended = frontmatter
        .get("recommended")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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
        recommended,
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
    /// Failed to parse the Datalog source (syntax / IR construction error)
    Parse(String),
    /// Rule evaluation failed at runtime (engine execution error)
    Eval(String),
}

impl std::fmt::Display for InterpretError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpretError::Io(e) => write!(f, "Failed to read rules file: {}", e),
            InterpretError::Parse(e) => write!(f, "Failed to parse rules: {}", e),
            InterpretError::Eval(e) => write!(f, "Rule evaluation failed: {}", e),
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
        Severity::Info | Severity::Hint => {
            // `DiagnosticLevel` has no `Info` variant; `Hint` is the closest
            // available level (quieter than Warning). This is a lossy mapping:
            // both "info" and "hint" from the Datalog `diagnostic` relation end
            // up as `Hint` after severity promotion. A future `Info` variant in
            // `DiagnosticLevel` would allow an exact mapping.
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
    let program = Program::from_ast(ast).map_err(InterpretError::Parse)?;

    // Create engine, populate facts, and run to fixpoint.
    //
    // NOTE: JIT is intentionally disabled here. ascent-interpreter 0.1.1 has a bug where
    // the JIT compares interned String values by their intern IDs (u32) using packed integer
    // comparison rather than lexicographic string content. This produces incorrect results for
    // rules using `if a < b` with String columns (e.g. `cycle(a, b) <-- ..., if a < b`).
    // Re-enable when ascent-interpreter fixes JIT string comparison.
    let mut engine = Engine::new(program);
    populate_facts(&mut engine, relations)?;
    engine
        .run()
        .map_err(|e| InterpretError::Eval(e.to_string()))?;
    engine.materialize();

    // Extract diagnostics from output relations
    Ok(extract_diagnostics(&engine))
}

/// Run rules incrementally against an already-populated engine.
///
/// For the daemon/LSP path where facts change between runs. The engine must
/// have been created and run at least once (via [`run_rules_source`] or
/// [`run_rules_source_incremental`]). New/changed facts are inserted then
/// `run_incremental` re-evaluates only strata affected by the changed relations.
///
/// `dirty_relations` names the input relations that have new facts inserted via
/// `new_relations`. `retracted_relations` names any relations that had facts removed
/// (triggering full clear-and-rederive for affected strata).
///
/// Returns diagnostics from the incremental re-evaluation.
pub fn run_rules_source_incremental(
    engine: &mut Engine,
    new_relations: &Relations,
    dirty_relations: &[&str],
    retracted_relations: &[&str],
) -> Result<Vec<Diagnostic>, InterpretError> {
    populate_facts(engine, new_relations)?;
    engine
        .run_incremental(dirty_relations, retracted_relations)
        .map_err(|e| InterpretError::Eval(e.to_string()))?;
    engine.materialize();
    Ok(extract_diagnostics(engine))
}

// =============================================================================
// Incremental evaluation: cached engine with source-tagged facts
// =============================================================================

/// A primed engine that supports incremental re-evaluation.
///
/// Holds an `Engine` whose input facts are tagged per source file so that facts
/// for changed files can be retracted and re-inserted before calling
/// [`run_incremental`](Engine::run_incremental).  The `rule_source_hash` field
/// is used by callers to detect when the rule source has changed and the cache
/// must be discarded.
pub struct CachedRuleEngine {
    /// The underlying interpreter engine, already primed with source-tagged facts.
    pub engine: Engine,
    /// Stable hash of the rule source string used to prime this engine.
    /// If the `.dl` source changes, callers must discard the cache.
    pub rule_source_hash: u64,
}

impl CachedRuleEngine {
    /// Compute a stable hash of a rule source string.
    pub fn hash_source(source: &str) -> u64 {
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        h.finish()
    }
}

/// Prime an engine for a rule, populating all facts with per-file source tags.
///
/// Runs a full evaluation and returns a `CachedRuleEngine` ready for subsequent
/// incremental re-evaluations via [`run_rule_incremental`].
pub fn prime_rule_engine(
    rule: &FactsRule,
    relations: &Relations,
) -> Result<CachedRuleEngine, InterpretError> {
    let full_source = format!("{}\n{}", PREAMBLE, &rule.source);
    let ast: AscentProgram =
        syn::parse_str(&full_source).map_err(|e| InterpretError::Parse(e.to_string()))?;
    let program = Program::from_ast(ast).map_err(InterpretError::Parse)?;

    let mut engine = Engine::new(program);
    populate_facts_with_sources(&mut engine, relations)?;
    engine
        .run()
        .map_err(|e| InterpretError::Eval(e.to_string()))?;
    engine.materialize();

    Ok(CachedRuleEngine {
        engine,
        rule_source_hash: CachedRuleEngine::hash_source(&rule.source),
    })
}

/// Run a rule incrementally against a cached engine.
///
/// `changed_files` are the relative file paths whose facts have changed (added,
/// modified, or deleted).  For each changed file this function:
/// 1. Retracts all facts tagged with that file's source ID.
/// 2. Re-inserts the file's current facts from `new_relations`, tagged with its source ID.
///
/// Then calls [`Engine::run_incremental`] over the affected input relations.
///
/// The set of "dirty" input relations is inferred by inspecting which fact
/// types have rows referencing any of the changed files.  The set of
/// "retracted" relations is always a superset of dirty (since we always retract
/// before re-inserting), so retracted == dirty.
///
/// Returns diagnostics from the incremental re-evaluation.
pub fn run_rule_incremental(
    cached: &mut CachedRuleEngine,
    new_relations: &Relations,
    changed_files: &[&str],
) -> Result<Vec<Diagnostic>, InterpretError> {
    if changed_files.is_empty() {
        // Nothing changed — return existing diagnostics without re-running.
        return Ok(extract_diagnostics(&cached.engine));
    }

    let changed_set: std::collections::HashSet<&str> = changed_files.iter().copied().collect();

    // Retract all facts for the changed files.
    let mut sources_to_retract: Vec<SourceId> = Vec::new();
    for file in changed_files {
        let source_id = cached.engine.intern_source(file);
        sources_to_retract.push(source_id);
    }
    cached.engine.retract_sources(sources_to_retract);

    // Track which input relations received new facts for any changed file.
    let mut dirty_input_relations: std::collections::HashSet<&'static str> =
        std::collections::HashSet::new();

    // Re-insert facts for changed files only (inline loops, no closures to avoid lifetime issues).
    for s in new_relations.symbols.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "symbol",
                    vec![
                        Value::string(&s.file),
                        Value::string(&s.name),
                        Value::string(&s.kind),
                        Value::U32(s.line),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("symbol");
        }
    }
    for s in new_relations.imports.iter() {
        if changed_set.contains(s.from_file.as_str()) {
            let sid = cached.engine.intern_source(s.from_file.as_str());
            cached
                .engine
                .insert_with_source(
                    "import",
                    vec![
                        Value::string(&s.from_file),
                        Value::string(&s.module_specifier),
                        Value::string(&s.name),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("import");
        }
    }
    for s in new_relations.calls.iter() {
        if changed_set.contains(s.caller_file.as_str()) {
            let sid = cached.engine.intern_source(s.caller_file.as_str());
            cached
                .engine
                .insert_with_source(
                    "call",
                    vec![
                        Value::string(&s.caller_file),
                        Value::string(&s.caller_name),
                        Value::string(&s.callee_name),
                        Value::U32(s.line),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("call");
        }
    }
    for s in new_relations.visibilities.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "visibility",
                    vec![
                        Value::string(&s.file),
                        Value::string(&s.name),
                        Value::string(&s.visibility),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("visibility");
        }
    }
    for s in new_relations.attributes.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "attribute",
                    vec![
                        Value::string(&s.file),
                        Value::string(&s.name),
                        Value::string(&s.attribute),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("attribute");
        }
    }
    for s in new_relations.parents.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "parent",
                    vec![
                        Value::string(&s.file),
                        Value::string(&s.child_name),
                        Value::string(&s.parent_name),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("parent");
        }
    }
    for s in new_relations.qualifiers.iter() {
        if changed_set.contains(s.caller_file.as_str()) {
            let sid = cached.engine.intern_source(s.caller_file.as_str());
            cached
                .engine
                .insert_with_source(
                    "qualifier",
                    vec![
                        Value::string(&s.caller_file),
                        Value::string(&s.caller_name),
                        Value::string(&s.callee_name),
                        Value::string(&s.qualifier),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("qualifier");
        }
    }
    for s in new_relations.symbol_ranges.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "symbol_range",
                    vec![
                        Value::string(&s.file),
                        Value::string(&s.name),
                        Value::U32(s.start_line),
                        Value::U32(s.end_line),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("symbol_range");
        }
    }
    for s in new_relations.implements.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "implements",
                    vec![
                        Value::string(&s.file),
                        Value::string(&s.name),
                        Value::string(&s.interface),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("implements");
        }
    }
    for s in new_relations.is_impls.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "is_impl",
                    vec![Value::string(&s.file), Value::string(&s.name)],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("is_impl");
        }
    }
    for s in new_relations.type_methods.iter() {
        if changed_set.contains(s.file.as_str()) {
            let sid = cached.engine.intern_source(s.file.as_str());
            cached
                .engine
                .insert_with_source(
                    "type_method",
                    vec![
                        Value::string(&s.file),
                        Value::string(&s.type_name),
                        Value::string(&s.method_name),
                    ],
                    sid,
                )
                .map_err(|e| InterpretError::Parse(e.to_string()))?;
            dirty_input_relations.insert("type_method");
        }
    }
    // All dirty input relations are also retracted (we retracted + re-inserted).
    let dirty_vec: Vec<&str> = dirty_input_relations.iter().copied().collect();
    cached
        .engine
        .run_incremental(&dirty_vec, &dirty_vec)
        .map_err(|e| InterpretError::Eval(e.to_string()))?;
    cached.engine.materialize();

    Ok(extract_diagnostics(&cached.engine))
}

/// Run a FactsRule with a cached engine, using incremental evaluation when possible.
///
/// On the first call (or when the cache is `None`), performs a full evaluation via
/// [`prime_rule_engine`] and stores the result in `*cached_engine`.
///
/// On subsequent calls with a warm cache, delegates to [`run_rule_incremental`]
/// if `changed_files` is non-empty.  If `changed_files` is empty the cached
/// diagnostics are returned directly without re-running the engine.
///
/// If the rule source has changed since the engine was primed (detected via hash),
/// the cache is discarded and a full re-evaluation is performed.
///
/// `changed_files` are relative file paths (the same strings used as keys in the
/// index).  Pass an empty slice to skip incremental eval and return cached results.
pub fn run_rule_with_cache(
    cached_engine: &mut Option<CachedRuleEngine>,
    rule: &FactsRule,
    relations: &Relations,
    changed_files: &[&str],
) -> Result<Vec<Diagnostic>, InterpretError> {
    let expected_hash = CachedRuleEngine::hash_source(&rule.source);

    // Discard stale cache when rule source changed.
    if let Some(cached) = cached_engine.as_ref()
        && cached.rule_source_hash != expected_hash
    {
        tracing::debug!(
            rule_id = %rule.id,
            "rule source changed — discarding engine cache"
        );
        *cached_engine = None;
    }

    if cached_engine.is_none() {
        // Cold path: prime the engine with a full evaluation.
        tracing::debug!(rule_id = %rule.id, "priming incremental engine (full eval)");
        let primed = prime_rule_engine(rule, relations)?;
        let diagnostics = extract_diagnostics(&primed.engine);
        *cached_engine = Some(primed);
        return Ok(diagnostics);
    }

    // Warm path: incremental re-evaluation.
    let cached = cached_engine.as_mut().unwrap();
    tracing::debug!(
        rule_id = %rule.id,
        changed = changed_files.len(),
        "running incremental Datalog eval"
    );
    run_rule_incremental(cached, relations, changed_files)
}

/// Run multiple rules against the same relations, sharing a single JIT compiler.
///
/// For the batch path (running all rules against the same index snapshot), this
/// compiles the first rule's program with JIT, then shares the compiled JIT state
/// across all subsequent engine instances — avoiding repeated JIT compilation of
/// identical rule bodies.
///
/// NOTE: JIT compiler sharing is structured but JIT is currently disabled pending an
/// ascent-interpreter bug fix (see `run_rules_source` for details). The `shared_jit`
/// infrastructure will become active once JIT is re-enabled.
///
/// Applies per-rule allow patterns and severity promotion, identical to `run_rule`.
/// Returns all diagnostics from all rules combined.
pub fn run_rules_batch(
    rules: &[&FactsRule],
    relations: &Relations,
) -> Result<Vec<Diagnostic>, InterpretError> {
    if rules.is_empty() {
        return Ok(Vec::new());
    }

    let mut _shared_jit: Option<SharedJitCompiler> = None;
    let mut all_diagnostics = Vec::new();

    for rule in rules {
        let full_source = format!("{}\n{}", PREAMBLE, &rule.source);
        let ast: AscentProgram =
            syn::parse_str(&full_source).map_err(|e| InterpretError::Parse(e.to_string()))?;
        let program = Program::from_ast(ast).map_err(InterpretError::Parse)?;

        let mut engine = Engine::new(program);

        // JIT sharing: disabled until ascent-interpreter JIT string comparison is fixed.
        // Uncomment to enable once the upstream bug is resolved:
        //   match _shared_jit.take() {
        //       Some(jit) => engine.set_jit_compiler(jit),
        //       None => engine.enable_jit().map_err(|e| InterpretError::Parse(e.to_string()))?,
        //   }

        populate_facts(&mut engine, relations)?;
        engine
            .run()
            .map_err(|e| InterpretError::Eval(e.to_string()))?;
        engine.materialize();

        // Capture the shared JIT for when it's re-enabled.
        _shared_jit = engine.share_jit_compiler();

        let mut diagnostics = extract_diagnostics(&engine);

        // Apply per-rule allow patterns.
        if !rule.allow.is_empty() {
            diagnostics.retain(|d| {
                let match_str = match d.location.as_ref() {
                    ROption::RSome(loc) => loc.file.as_str(),
                    ROption::RNone => d.message.as_str(),
                };
                !rule.allow.iter().any(|p| p.matches(match_str))
            });
        }

        // Apply per-rule severity.
        match rule.severity {
            Severity::Error => {
                for d in &mut diagnostics {
                    d.level = DiagnosticLevel::Error;
                }
            }
            Severity::Info | Severity::Hint => {
                // `DiagnosticLevel` has no `Info` variant; `Hint` is the closest
                // available level. See the same comment in `run_rule` for details.
                for d in &mut diagnostics {
                    if d.level == DiagnosticLevel::Warning {
                        d.level = DiagnosticLevel::Hint;
                    }
                }
            }
            Severity::Warning => {}
        }

        all_diagnostics.extend(diagnostics);
    }

    Ok(all_diagnostics)
}

/// Populate the engine with facts from Relations, tagging each fact with a per-file source ID.
///
/// Used by [`prime_rule_engine`] to enable source-based retraction for incremental evaluation.
/// Each fact's file path is interned as a source name so that facts for a changed file can be
/// retracted with [`Engine::retract_sources`] before re-inserting the updated facts.
fn populate_facts_with_sources(
    engine: &mut Engine,
    relations: &Relations,
) -> Result<(), InterpretError> {
    for s in relations.symbols.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "symbol",
                vec![
                    Value::string(&s.file),
                    Value::string(&s.name),
                    Value::string(&s.kind),
                    Value::U32(s.line),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.imports.iter() {
        let sid = engine.intern_source(s.from_file.as_str());
        engine
            .insert_with_source(
                "import",
                vec![
                    Value::string(&s.from_file),
                    Value::string(&s.module_specifier),
                    Value::string(&s.name),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.calls.iter() {
        let sid = engine.intern_source(s.caller_file.as_str());
        engine
            .insert_with_source(
                "call",
                vec![
                    Value::string(&s.caller_file),
                    Value::string(&s.caller_name),
                    Value::string(&s.callee_name),
                    Value::U32(s.line),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.visibilities.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "visibility",
                vec![
                    Value::string(&s.file),
                    Value::string(&s.name),
                    Value::string(&s.visibility),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.attributes.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "attribute",
                vec![
                    Value::string(&s.file),
                    Value::string(&s.name),
                    Value::string(&s.attribute),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.parents.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "parent",
                vec![
                    Value::string(&s.file),
                    Value::string(&s.child_name),
                    Value::string(&s.parent_name),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.qualifiers.iter() {
        let sid = engine.intern_source(s.caller_file.as_str());
        engine
            .insert_with_source(
                "qualifier",
                vec![
                    Value::string(&s.caller_file),
                    Value::string(&s.caller_name),
                    Value::string(&s.callee_name),
                    Value::string(&s.qualifier),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.symbol_ranges.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "symbol_range",
                vec![
                    Value::string(&s.file),
                    Value::string(&s.name),
                    Value::U32(s.start_line),
                    Value::U32(s.end_line),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.implements.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "implements",
                vec![
                    Value::string(&s.file),
                    Value::string(&s.name),
                    Value::string(&s.interface),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.is_impls.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "is_impl",
                vec![Value::string(&s.file), Value::string(&s.name)],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    for s in relations.type_methods.iter() {
        let sid = engine.intern_source(s.file.as_str());
        engine
            .insert_with_source(
                "type_method",
                vec![
                    Value::string(&s.file),
                    Value::string(&s.type_name),
                    Value::string(&s.method_name),
                ],
                sid,
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }
    Ok(())
}

/// Populate the engine with facts from Relations.
fn populate_facts(engine: &mut Engine, relations: &Relations) -> Result<(), InterpretError> {
    for sym in relations.symbols.iter() {
        engine
            .insert(
                "symbol",
                vec![
                    Value::string(&sym.file),
                    Value::string(&sym.name),
                    Value::string(&sym.kind),
                    Value::U32(sym.line),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for imp in relations.imports.iter() {
        engine
            .insert(
                "import",
                vec![
                    Value::string(&imp.from_file),
                    Value::string(&imp.module_specifier),
                    Value::string(&imp.name),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for call in relations.calls.iter() {
        engine
            .insert(
                "call",
                vec![
                    Value::string(&call.caller_file),
                    Value::string(&call.caller_name),
                    Value::string(&call.callee_name),
                    Value::U32(call.line),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for vis in relations.visibilities.iter() {
        engine
            .insert(
                "visibility",
                vec![
                    Value::string(&vis.file),
                    Value::string(&vis.name),
                    Value::string(&vis.visibility),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for attr in relations.attributes.iter() {
        engine
            .insert(
                "attribute",
                vec![
                    Value::string(&attr.file),
                    Value::string(&attr.name),
                    Value::string(&attr.attribute),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for p in relations.parents.iter() {
        engine
            .insert(
                "parent",
                vec![
                    Value::string(&p.file),
                    Value::string(&p.child_name),
                    Value::string(&p.parent_name),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for q in relations.qualifiers.iter() {
        engine
            .insert(
                "qualifier",
                vec![
                    Value::string(&q.caller_file),
                    Value::string(&q.caller_name),
                    Value::string(&q.callee_name),
                    Value::string(&q.qualifier),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for sr in relations.symbol_ranges.iter() {
        engine
            .insert(
                "symbol_range",
                vec![
                    Value::string(&sr.file),
                    Value::string(&sr.name),
                    Value::U32(sr.start_line),
                    Value::U32(sr.end_line),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for imp in relations.implements.iter() {
        engine
            .insert(
                "implements",
                vec![
                    Value::string(&imp.file),
                    Value::string(&imp.name),
                    Value::string(&imp.interface),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for ii in relations.is_impls.iter() {
        engine
            .insert(
                "is_impl",
                vec![Value::string(&ii.file), Value::string(&ii.name)],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    for tm in relations.type_methods.iter() {
        engine
            .insert(
                "type_method",
                vec![
                    Value::string(&tm.file),
                    Value::string(&tm.type_name),
                    Value::string(&tm.method_name),
                ],
            )
            .map_err(|e| InterpretError::Parse(e.to_string()))?;
    }

    Ok(())
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
            if let [severity, rule_id, file, Value::U32(line), message] = tuple {
                let (Some(severity), Some(rule_id), Some(file), Some(message)) = (
                    severity.as_str(),
                    rule_id.as_str(),
                    file.as_str(),
                    message.as_str(),
                ) else {
                    continue;
                };
                let mut d = match severity {
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
