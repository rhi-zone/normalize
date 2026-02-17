//! Unified rule management - list, run, add, update, remove rules (syntax + fact).

use crate::output::OutputFormat;
use clap::Subcommand;
use normalize_facts_rules_interpret as interpret;
use normalize_syntax_rules::{self, DebugFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Rule type filter for list/run commands.
#[derive(Clone, Debug, Default, clap::ValueEnum, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleType {
    #[default]
    All,
    Syntax,
    Fact,
}

#[derive(Subcommand, Deserialize, schemars::JsonSchema)]
pub enum RulesAction {
    /// List all rules (syntax + fact, builtin + user)
    List {
        /// Show source URLs for imported rules
        #[arg(long)]
        #[serde(default)]
        sources: bool,

        /// Filter by rule type
        #[arg(long, default_value = "all")]
        #[serde(default)]
        r#type: RuleType,
    },

    /// Run rules against the codebase
    Run {
        /// Specific rule ID to run
        #[arg(long)]
        rule: Option<String>,

        /// Apply auto-fixes (syntax rules only)
        #[arg(long)]
        #[serde(default)]
        fix: bool,

        /// Output in SARIF format
        #[arg(long)]
        #[serde(default)]
        sarif: bool,

        /// Target directory or file
        target: Option<String>,

        /// Filter by rule type
        #[arg(long, default_value = "all")]
        #[serde(default)]
        r#type: RuleType,

        /// Debug flags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        #[serde(default)]
        debug: Vec<String>,
    },

    /// Add a rule from a URL
    Add {
        /// URL to download the rule from
        url: String,

        /// Install to global rules (~/.config/normalize/rules/) instead of project
        #[arg(long)]
        #[serde(default)]
        global: bool,
    },

    /// Update imported rules from their sources
    Update {
        /// Specific rule ID to update (updates all if omitted)
        rule_id: Option<String>,
    },

    /// Remove an imported rule
    Remove {
        /// Rule ID to remove
        rule_id: String,
    },
}

/// Lock file entry tracking an imported rule
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuleLockEntry {
    source: String,
    sha256: String,
    added: String,
}

/// Lock file format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RulesLock {
    rules: HashMap<String, RuleLockEntry>,
}

impl RulesLock {
    fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &Path) -> std::io::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        std::fs::write(path, content)
    }
}

/// Run the rules command
pub fn cmd_rules(action: RulesAction, root: Option<&Path>, format: &OutputFormat) -> i32 {
    let effective_root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let config = crate::config::NormalizeConfig::load(&effective_root);
    let json = format.is_json();

    match action {
        RulesAction::List { sources, r#type } => {
            cmd_list(&effective_root, sources, &r#type, json, &config)
        }
        RulesAction::Run {
            rule,
            fix,
            sarif,
            target,
            r#type,
            debug,
        } => {
            let target_root = target
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| effective_root.clone());
            cmd_run(
                &target_root,
                rule.as_deref(),
                fix,
                sarif,
                &r#type,
                &debug,
                json,
                &config,
            )
        }
        RulesAction::Add { url, global } => cmd_add(&url, global, json),
        RulesAction::Update { rule_id } => cmd_update(rule_id.as_deref(), json),
        RulesAction::Remove { rule_id } => cmd_remove(&rule_id, json),
    }
}

/// Run syntax rules only (called from `normalize analyze rules`).
#[allow(clippy::too_many_arguments)]
pub fn cmd_run_syntax(
    root: &Path,
    filter_rule: Option<&str>,
    list_only: bool,
    fix: bool,
    format: &OutputFormat,
    sarif: bool,
    config: &normalize_syntax_rules::RulesConfig,
    debug: &DebugFlags,
) -> i32 {
    crate::commands::analyze::rules_cmd::cmd_rules(
        root,
        filter_rule,
        list_only,
        fix,
        format,
        sarif,
        config,
        debug,
    )
}

/// Run fact rules only (called from `normalize facts check`).
pub fn cmd_run_facts(
    root: &Path,
    rules_file: Option<&Path>,
    list_only: bool,
    json: bool,
    config: &interpret::FactsRulesConfig,
) -> i32 {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(run_fact_rules(root, rules_file, list_only, json, config))
}

// =============================================================================
// Unified List
// =============================================================================

/// Unified rule descriptor for display.
struct UnifiedRule {
    id: String,
    rule_type: &'static str,
    severity: String,
    source: &'static str,
    message: String,
    enabled: bool,
}

fn cmd_list(
    root: &Path,
    sources: bool,
    type_filter: &RuleType,
    json: bool,
    config: &crate::config::NormalizeConfig,
) -> i32 {
    let mut all_rules = Vec::new();

    // Load syntax rules
    if matches!(type_filter, RuleType::All | RuleType::Syntax) {
        let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.analyze.rules);
        for r in &syntax_rules {
            let source = if r.builtin { "builtin" } else { "project" };
            all_rules.push(UnifiedRule {
                id: r.id.clone(),
                rule_type: "syntax",
                severity: r.severity.to_string(),
                source,
                message: r.message.clone(),
                enabled: r.enabled,
            });
        }
    }

    // Load fact rules
    if matches!(type_filter, RuleType::All | RuleType::Fact) {
        let fact_rules = interpret::load_all_rules(root, &config.analyze.facts_rules);
        for r in &fact_rules {
            let source = if r.builtin { "builtin" } else { "project" };
            all_rules.push(UnifiedRule {
                id: r.id.clone(),
                rule_type: "fact",
                severity: r.severity.to_string(),
                source,
                message: r.message.clone(),
                enabled: r.enabled,
            });
        }
    }

    // Sort by type then id for stable output
    all_rules.sort_by(|a, b| a.rule_type.cmp(b.rule_type).then(a.id.cmp(&b.id)));

    if json {
        let rules_json: Vec<_> = all_rules
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "type": r.rule_type,
                    "severity": r.severity,
                    "source": r.source,
                    "message": r.message,
                    "enabled": r.enabled,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rules_json).unwrap());
    } else if all_rules.is_empty() {
        println!("No rules found.");
    } else {
        let syntax_count = all_rules.iter().filter(|r| r.rule_type == "syntax").count();
        let fact_count = all_rules.iter().filter(|r| r.rule_type == "fact").count();
        let disabled_count = all_rules.iter().filter(|r| !r.enabled).count();

        if disabled_count > 0 {
            println!(
                "{} rules ({} syntax, {} fact) — {} disabled\n",
                all_rules.len(),
                syntax_count,
                fact_count,
                disabled_count
            );
        } else {
            println!(
                "{} rules ({} syntax, {} fact)\n",
                all_rules.len(),
                syntax_count,
                fact_count
            );
        }

        for r in &all_rules {
            let disabled_marker = if r.enabled { "" } else { "  [disabled]" };
            if sources {
                println!(
                    "  [{}]  {:30} {:9} {:7}  {}{}",
                    r.rule_type, r.id, r.severity, r.source, r.message, disabled_marker
                );
            } else {
                println!(
                    "  [{}]  {:30} {:9} {}{}",
                    r.rule_type, r.id, r.severity, r.message, disabled_marker
                );
            }
        }
    }

    0
}

// =============================================================================
// Unified Run
// =============================================================================

#[allow(clippy::too_many_arguments)]
fn cmd_run(
    root: &Path,
    filter_rule: Option<&str>,
    fix: bool,
    sarif: bool,
    type_filter: &RuleType,
    debug: &[String],
    json: bool,
    config: &crate::config::NormalizeConfig,
) -> i32 {
    let mut exit_code = 0;

    // Run syntax rules
    if matches!(type_filter, RuleType::All | RuleType::Syntax) {
        let debug_flags = DebugFlags::from_args(debug);
        let format = if json {
            OutputFormat::Json
        } else {
            OutputFormat::default()
        };
        let code = crate::commands::analyze::rules_cmd::cmd_rules(
            root,
            filter_rule,
            false,
            fix,
            &format,
            sarif,
            &config.analyze.rules,
            &debug_flags,
        );
        if code != 0 {
            exit_code = code;
        }
    }

    // Run fact rules
    if matches!(type_filter, RuleType::All | RuleType::Fact) {
        let code = cmd_run_facts(root, None, false, json, &config.analyze.facts_rules);
        if code != 0 {
            exit_code = code;
        }
    }

    exit_code
}

/// Run fact rules (interpreted) against the index.
async fn run_fact_rules(
    root: &Path,
    rules_file: Option<&Path>,
    list_only: bool,
    json: bool,
    config: &interpret::FactsRulesConfig,
) -> i32 {
    // If a specific file is given, run just that file
    if let Some(path) = rules_file {
        return run_fact_rules_file(root, path, json).await;
    }

    // Auto-discover rules with config overrides
    let all_rules_unfiltered = interpret::load_all_rules(root, config);

    if list_only {
        let all_rules = &all_rules_unfiltered;
        if json {
            let rules_json: Vec<_> = all_rules
                .iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "message": r.message,
                        "severity": r.severity.to_string(),
                        "builtin": r.builtin,
                        "source_path": if r.source_path.as_os_str().is_empty() {
                            None
                        } else {
                            Some(r.source_path.display().to_string())
                        },
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&rules_json).unwrap());
        } else {
            let builtin_count = all_rules.iter().filter(|r| r.builtin).count();
            let project_count = all_rules.len() - builtin_count;
            println!(
                "{} fact rules ({} builtin, {} project)",
                all_rules.len(),
                builtin_count,
                project_count
            );
            println!();
            for rule in all_rules {
                let source = if rule.builtin { "builtin" } else { "project" };
                println!(
                    "  {:30} [{}] {} {}",
                    rule.id, source, rule.severity, rule.message
                );
            }
        }
        return 0;
    }

    // Filter to enabled rules for execution
    let all_rules: Vec<_> = all_rules_unfiltered
        .into_iter()
        .filter(|r| r.enabled)
        .collect();

    if all_rules.is_empty() {
        println!("No fact rules found.");
        return 0;
    }

    // Build relations from index (auto-build if missing)
    let relations = match ensure_relations(root).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error building relations: {}", e);
            return 1;
        }
    };

    // Run all rules
    let mut all_diagnostics = Vec::new();
    let use_colors = !json && std::io::stdout().is_terminal();

    for rule in &all_rules {
        match interpret::run_rule(rule, &relations) {
            Ok(diagnostics) => all_diagnostics.extend(diagnostics),
            Err(e) => {
                eprintln!("Error running rule '{}': {}", rule.id, e);
            }
        }
    }

    // Filter inline normalize-facts-allow: comments in source files
    interpret::filter_inline_allowed(&mut all_diagnostics, root);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&all_diagnostics).unwrap()
        );
    } else if all_diagnostics.is_empty() {
        println!("No issues found ({} rules checked).", all_rules.len());
    } else {
        for diag in &all_diagnostics {
            println!("{}", crate::rules::format_diagnostic(diag, use_colors));
        }
        println!(
            "\n{} issue(s) found ({} rules checked).",
            all_diagnostics.len(),
            all_rules.len()
        );
    }

    if all_diagnostics
        .iter()
        .any(|d| d.level == normalize_facts_rules_api::DiagnosticLevel::Error)
    {
        1
    } else {
        0
    }
}

/// Build relations from the index, auto-building the index if it doesn't exist.
async fn ensure_relations(root: &Path) -> Result<normalize_facts_rules_api::Relations, String> {
    match super::facts::build_relations_from_index(root).await {
        Ok(r) => Ok(r),
        Err(_) => {
            eprintln!("Facts index not found. Building...");
            let mut idx = crate::index::open(root)
                .await
                .map_err(|e| format!("Failed to open index: {}", e))?;
            let count = idx
                .refresh()
                .await
                .map_err(|e| format!("Failed to index files: {}", e))?;
            eprintln!("Indexed {} files.", count);
            let stats = idx
                .refresh_call_graph()
                .await
                .map_err(|e| format!("Failed to index call graph: {}", e))?;
            eprintln!(
                "Indexed {} symbols, {} calls, {} imports.",
                stats.symbols, stats.calls, stats.imports
            );
            super::facts::build_relations_from_index(root).await
        }
    }
}

/// Run a single .dl file directly (explicit path mode)
async fn run_fact_rules_file(root: &Path, rules_file: &Path, json: bool) -> i32 {
    let relations = match ensure_relations(root).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error building relations: {}", e);
            return 1;
        }
    };

    let diagnostics = match interpret::run_rules_file(rules_file, &relations) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            return 1;
        }
    };

    let use_colors = !json && std::io::stdout().is_terminal();

    if json {
        println!("{}", serde_json::to_string_pretty(&diagnostics).unwrap());
    } else if diagnostics.is_empty() {
        println!("No issues found.");
    } else {
        for diag in &diagnostics {
            println!("{}", crate::rules::format_diagnostic(diag, use_colors));
        }
        println!("\n{} issue(s) found.", diagnostics.len());
    }

    if diagnostics
        .iter()
        .any(|d| d.level == normalize_facts_rules_api::DiagnosticLevel::Error)
    {
        1
    } else {
        0
    }
}

// =============================================================================
// Add / Update / Remove
// =============================================================================

fn rules_dir(global: bool) -> Option<PathBuf> {
    if global {
        dirs::config_dir().map(|d| d.join("normalize").join("rules"))
    } else {
        Some(PathBuf::from(".normalize").join("rules"))
    }
}

fn lock_file_path(global: bool) -> Option<PathBuf> {
    if global {
        dirs::config_dir().map(|d| d.join("normalize").join("rules.lock"))
    } else {
        Some(PathBuf::from(".normalize").join("rules.lock"))
    }
}

/// Detect rule file extension from URL or content.
fn detect_extension(url: &str) -> &'static str {
    if url.ends_with(".dl") { "dl" } else { "scm" }
}

fn cmd_add(url: &str, global: bool, json: bool) -> i32 {
    let Some(rules_dir) = rules_dir(global) else {
        eprintln!("Could not determine rules directory");
        return 1;
    };

    // Create rules directory if needed
    if let Err(e) = std::fs::create_dir_all(&rules_dir) {
        eprintln!("Failed to create rules directory: {}", e);
        return 1;
    }

    // Download the rule
    let content = match download_url(url) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to download rule: {}", e);
            return 1;
        }
    };

    // Extract rule ID from content
    let rule_id = match extract_rule_id(&content) {
        Some(id) => id,
        None => {
            eprintln!("Could not extract rule ID from downloaded content");
            eprintln!("Rule must have TOML frontmatter with 'id' field");
            return 1;
        }
    };

    // Detect extension from URL
    let ext = detect_extension(url);

    // Save rule file
    let rule_path = rules_dir.join(format!("{}.{}", rule_id, ext));
    if let Err(e) = std::fs::write(&rule_path, &content) {
        eprintln!("Failed to save rule: {}", e);
        return 1;
    }

    // Update lock file
    let Some(lock_path) = lock_file_path(global) else {
        eprintln!("Could not determine lock file path");
        return 1;
    };

    let mut lock = RulesLock::load(&lock_path);
    lock.rules.insert(
        rule_id.clone(),
        RuleLockEntry {
            source: url.to_string(),
            sha256: sha256_hex(&content),
            added: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        },
    );

    if let Err(e) = lock.save(&lock_path) {
        eprintln!("Warning: Failed to update lock file: {}", e);
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "added": rule_id,
                "path": rule_path,
                "source": url
            })
        );
    } else {
        println!("Added rule '{}' from {}", rule_id, url);
        println!("Saved to: {}", rule_path.display());
    }

    0
}

fn cmd_update(rule_id: Option<&str>, json: bool) -> i32 {
    let mut updated = Vec::new();
    let mut errors = Vec::new();

    for global in [false, true] {
        if let (Some(lock_path), Some(rules_dir)) = (lock_file_path(global), rules_dir(global)) {
            let lock = RulesLock::load(&lock_path);
            for (id, entry) in &lock.rules {
                if rule_id.is_some() && rule_id != Some(id.as_str()) {
                    continue;
                }
                match download_url(&entry.source) {
                    Ok(content) => {
                        let ext = detect_extension(&entry.source);
                        let path = rules_dir.join(format!("{}.{}", id, ext));
                        if let Err(e) = std::fs::write(&path, &content) {
                            errors.push((id.clone(), e.to_string()));
                        } else {
                            updated.push(id.clone());
                        }
                    }
                    Err(e) => {
                        errors.push((id.clone(), e.to_string()));
                    }
                }
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "updated": updated,
                "errors": errors
            })
        );
    } else if updated.is_empty() && errors.is_empty() {
        println!("No imported rules to update.");
    } else {
        for id in &updated {
            println!("Updated: {}", id);
        }
        for (id, err) in &errors {
            eprintln!("Failed to update {}: {}", id, err);
        }
    }

    if errors.is_empty() { 0 } else { 1 }
}

fn cmd_remove(rule_id: &str, json: bool) -> i32 {
    let mut removed = false;

    for global in [false, true] {
        if removed {
            break;
        }
        if let (Some(lock_path), Some(rules_dir)) = (lock_file_path(global), rules_dir(global)) {
            let mut lock = RulesLock::load(&lock_path);
            if lock.rules.remove(rule_id).is_some() {
                let _ = lock.save(&lock_path);
                // Try both extensions
                for ext in ["scm", "dl"] {
                    let rule_path = rules_dir.join(format!("{}.{}", rule_id, ext));
                    let _ = std::fs::remove_file(&rule_path);
                }
                removed = true;
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "removed": removed,
                "rule_id": rule_id
            })
        );
    } else if removed {
        println!("Removed rule '{}'", rule_id);
    } else {
        eprintln!("Rule '{}' not found in lock file", rule_id);
        return 1;
    }

    0
}

// =============================================================================
// Helpers
// =============================================================================

fn download_url(url: &str) -> Result<String, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if response.status() != 200 {
        return Err(format!(
            "HTTP {}: {}",
            response.status(),
            response.status_text()
        ));
    }

    response
        .into_string()
        .map_err(|e| format!("Failed to read response: {}", e))
}

fn extract_rule_id(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();

    let mut in_frontmatter = false;
    let mut toml_lines = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "# ---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if let Some(rest) = trimmed.strip_prefix("# ") {
                toml_lines.push(rest);
            } else if let Some(rest) = trimmed.strip_prefix('#') {
                toml_lines.push(rest);
            }
        }
    }

    if toml_lines.is_empty() {
        return None;
    }

    let toml_content = toml_lines.join("\n");
    let table: toml::Table = toml_content.parse().ok()?;
    table.get("id")?.as_str().map(|s| s.to_string())
}

fn sha256_hex(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
