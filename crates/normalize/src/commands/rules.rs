//! Unified rule management - list, run, add, update, remove rules (syntax + fact).

use crate::output::OutputFormat;
use clap::Subcommand;
use normalize_facts_rules_interpret as interpret;
use normalize_syntax_rules::{self, DebugFlags};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

        /// Filter by tag (e.g. "debug-print", "security")
        #[arg(long)]
        tag: Option<String>,

        /// Filter to enabled rules only
        #[arg(long)]
        #[serde(default)]
        enabled: bool,

        /// Filter to disabled rules only
        #[arg(long)]
        #[serde(default)]
        disabled: bool,

        /// Hide the description line (compact one-line-per-rule output)
        #[arg(long)]
        #[serde(default)]
        no_desc: bool,
    },

    /// Run rules against the codebase
    Run {
        /// Specific rule ID to run
        #[arg(long)]
        rule: Option<String>,

        /// Filter by tag (e.g. "debug-print", "security")
        #[arg(long)]
        tag: Option<String>,

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

    /// Enable a rule or all rules matching a tag
    Enable {
        /// Rule ID or tag name
        id_or_tag: String,

        /// Preview changes without writing
        #[arg(long)]
        #[serde(default)]
        dry_run: bool,
    },

    /// Disable a rule or all rules matching a tag
    Disable {
        /// Rule ID or tag name
        id_or_tag: String,

        /// Preview changes without writing
        #[arg(long)]
        #[serde(default)]
        dry_run: bool,
    },

    /// Show full documentation for a rule
    Show {
        /// Rule ID to show
        id: String,
    },

    /// List all tags and the rules they group
    Tags {
        /// Expand each tag to show its member rules
        #[arg(long)]
        #[serde(default)]
        show_rules: bool,

        /// Show only this specific tag
        #[arg(long)]
        tag: Option<String>,
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
    let use_colors = format.use_colors();

    match action {
        RulesAction::List {
            sources,
            r#type,
            tag,
            enabled,
            disabled,
            no_desc,
        } => cmd_list(
            &effective_root,
            ListFilters {
                sources,
                type_filter: &r#type,
                tag: tag.as_deref(),
                enabled,
                disabled,
                no_desc,
                json,
                use_colors,
            },
            &config,
        ),
        RulesAction::Run {
            rule,
            tag,
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
                tag.as_deref(),
                fix,
                sarif,
                &r#type,
                &debug,
                json,
                &config,
            )
        }
        RulesAction::Enable { id_or_tag, dry_run } => {
            cmd_enable_disable(&effective_root, &id_or_tag, true, dry_run, &config)
        }
        RulesAction::Disable { id_or_tag, dry_run } => {
            cmd_enable_disable(&effective_root, &id_or_tag, false, dry_run, &config)
        }
        RulesAction::Show { id } => cmd_show(&effective_root, &id, json, use_colors, &config),
        RulesAction::Tags { show_rules, tag } => cmd_tags(
            &effective_root,
            show_rules,
            tag.as_deref(),
            json,
            use_colors,
            &config,
        ),
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
        None,
        None,
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
    rt.block_on(run_fact_rules(
        root, rules_file, list_only, json, config, None,
    ))
}

// =============================================================================
// Tag colors
// =============================================================================

/// Deterministic color for a tag name.
///
/// Uses a stable hash of the tag string to pick from a curated palette.
/// Red and yellow are reserved for severity indicators and are never used.
fn tag_color(tag: &str) -> nu_ansi_term::Color {
    use nu_ansi_term::Color;
    // Curated palette: readable on both dark and light terminals.
    // Red / Yellow omitted — reserved for error / warning severity.
    const PALETTE: &[Color] = &[
        Color::Cyan,
        Color::Green,
        Color::Blue,
        Color::Magenta,
        Color::Fixed(93),  // purple
        Color::Fixed(37),  // teal
        Color::Fixed(67),  // steel blue
        Color::Fixed(107), // olive green
        Color::Fixed(135), // medium purple
        Color::Fixed(73),  // cadet blue
    ];
    // FNV-1a hash for a stable, fast, dependency-free result
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in tag.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    PALETTE[(hash as usize) % PALETTE.len()]
}

/// Render a tag: colored name without brackets in pretty mode, `[tag]` in plain.
fn paint_tag(tag: &str, use_colors: bool) -> String {
    if use_colors {
        tag_color(tag).paint(tag).to_string()
    } else {
        format!("[{}]", tag)
    }
}

/// Color a severity string: error=red, warning=yellow, info=dark-gray.
fn paint_severity(severity: &str, use_colors: bool) -> String {
    if !use_colors {
        return severity.to_string();
    }
    match severity {
        "error" => nu_ansi_term::Color::Red.paint(severity).to_string(),
        "warning" => nu_ansi_term::Color::Yellow.paint(severity).to_string(),
        "info" => nu_ansi_term::Color::Blue.paint(severity).to_string(),
        _ => nu_ansi_term::Color::DarkGray.paint(severity).to_string(),
    }
}

/// Render a list of tags, space-separated, with optional colors.
fn paint_tags(tags: &[String], use_colors: bool) -> String {
    tags.iter()
        .map(|t| paint_tag(t, use_colors))
        .collect::<Vec<_>>()
        .join(" ")
}

// =============================================================================
// Tag expansion
// =============================================================================

/// Expand a tag name into the set of rule IDs it covers.
///
/// Resolution order:
/// 1. Check `[rule-tags]` for a user-defined group — each entry may be a rule ID
///    or another tag name (recursed, with cycle detection).
/// 2. Any rule whose `tags` list contains the tag name directly.
///
/// Both sources are unioned together.
fn expand_tag<'a>(
    tag: &str,
    rule_tags: &'a HashMap<String, Vec<String>>,
    all_rules: &'a [UnifiedRule],
    visited: &mut HashSet<String>,
) -> HashSet<&'a str> {
    if !visited.insert(tag.to_string()) {
        // Cycle — stop recursing
        return HashSet::new();
    }

    let mut ids: HashSet<&'a str> = HashSet::new();

    // Builtin/per-rule tags: rules that carry this tag directly
    for r in all_rules {
        if r.tags.iter().any(|t| t == tag) {
            ids.insert(r.id.as_str());
        }
    }

    // User-defined group
    if let Some(members) = rule_tags.get(tag) {
        for member in members {
            // Is it a rule ID?
            if all_rules.iter().any(|r| r.id == *member) {
                ids.insert(member.as_str());
            } else {
                // Treat as a tag reference — recurse
                ids.extend(expand_tag(member, rule_tags, all_rules, visited));
            }
        }
    }

    ids
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
    tags: Vec<String>,
}

struct ListFilters<'a> {
    sources: bool,
    type_filter: &'a RuleType,
    tag: Option<&'a str>,
    enabled: bool,
    disabled: bool,
    no_desc: bool,
    json: bool,
    use_colors: bool,
}

fn cmd_list(root: &Path, filters: ListFilters<'_>, config: &crate::config::NormalizeConfig) -> i32 {
    let ListFilters {
        sources,
        type_filter,
        tag: tag_filter,
        enabled: enabled_filter,
        disabled: disabled_filter,
        no_desc,
        json,
        use_colors,
    } = filters;
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
                tags: r.tags.clone(),
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
                tags: r.tags.clone(),
            });
        }
    }

    // Apply filters (all compose via AND)
    if let Some(tag) = tag_filter {
        let rule_tags = &config.rule_tags.0;
        let mut visited = HashSet::new();
        let matching_ids: HashSet<String> = expand_tag(tag, rule_tags, &all_rules, &mut visited)
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        all_rules.retain(|r| matching_ids.contains(&r.id));
    }
    if enabled_filter {
        all_rules.retain(|r| r.enabled);
    }
    if disabled_filter {
        all_rules.retain(|r| !r.enabled);
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
                    "tags": r.tags,
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
            let tags_str = if r.tags.is_empty() {
                String::new()
            } else {
                format!("  {}", paint_tags(&r.tags, use_colors))
            };
            let sev = paint_severity(&r.severity, use_colors);
            // First line: type, id, severity, (source), tags, disabled marker
            if sources {
                println!(
                    "  [{}]  {:30} {:9} {:7}{}{}",
                    r.rule_type, r.id, sev, r.source, tags_str, disabled_marker
                );
            } else {
                println!(
                    "  [{}]  {:30} {:9}{}{}",
                    r.rule_type, r.id, sev, tags_str, disabled_marker
                );
            }
            // Second line: description (suppressed by --no-desc)
            if !no_desc {
                println!("            {}", r.message);
            }
        }
    }

    0
}

// =============================================================================
// Enable / Disable
// =============================================================================

fn cmd_enable_disable(
    root: &Path,
    id_or_tag: &str,
    enable: bool,
    dry_run: bool,
    config: &crate::config::NormalizeConfig,
) -> i32 {
    // Resolve which rule IDs to affect
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.analyze.rules);
    let fact_rules = interpret::load_all_rules(root, &config.analyze.facts_rules);

    // Build unified list for tag expansion
    let all_unified: Vec<UnifiedRule> = syntax_rules
        .iter()
        .map(|r| UnifiedRule {
            id: r.id.clone(),
            rule_type: "syntax",
            severity: r.severity.to_string(),
            source: if r.builtin { "builtin" } else { "project" },
            message: r.message.clone(),
            enabled: r.enabled,
            tags: r.tags.clone(),
        })
        .chain(fact_rules.iter().map(|r| UnifiedRule {
            id: r.id.clone(),
            rule_type: "fact",
            severity: r.severity.to_string(),
            source: if r.builtin { "builtin" } else { "project" },
            message: r.message.clone(),
            enabled: r.enabled,
            tags: r.tags.clone(),
        }))
        .collect();

    // Exact ID match takes priority; otherwise expand as tag (includes user-defined groups)
    let rule_tags = &config.rule_tags.0;
    let matched_ids: HashSet<&str> = {
        if all_unified.iter().any(|r| r.id == id_or_tag) {
            // Exact ID match
            std::iter::once(id_or_tag).collect()
        } else {
            let mut visited = HashSet::new();
            expand_tag(id_or_tag, rule_tags, &all_unified, &mut visited)
        }
    };

    let matched_syntax: Vec<&normalize_syntax_rules::Rule> = syntax_rules
        .iter()
        .filter(|r| matched_ids.contains(r.id.as_str()))
        .collect();
    let matched_fact: Vec<&interpret::FactsRule> = fact_rules
        .iter()
        .filter(|r| matched_ids.contains(r.id.as_str()))
        .collect();

    if matched_syntax.is_empty() && matched_fact.is_empty() {
        eprintln!(
            "No rules found matching '{}' (not a rule ID or tag)",
            id_or_tag
        );
        return 1;
    }

    let verb = if enable { "enable" } else { "disable" };
    let config_path = root.join(".normalize").join("config.toml");

    // Collect all changes to apply: (section, rule_id)
    // syntax rules → [analyze.rules."id"]
    // fact rules   → [analyze.facts-rules."id"]
    let changes_syntax: Vec<&str> = matched_syntax
        .iter()
        .filter(|r| r.enabled != enable)
        .map(|r| r.id.as_str())
        .collect();
    let changes_fact: Vec<&str> = matched_fact
        .iter()
        .filter(|r| r.enabled != enable)
        .map(|r| r.id.as_str())
        .collect();

    // Rules already in the desired state
    let already_syntax: Vec<&str> = matched_syntax
        .iter()
        .filter(|r| r.enabled == enable)
        .map(|r| r.id.as_str())
        .collect();
    let already_fact: Vec<&str> = matched_fact
        .iter()
        .filter(|r| r.enabled == enable)
        .map(|r| r.id.as_str())
        .collect();

    for id in &already_syntax {
        println!("{}: already {}d (no change)", id, verb);
    }
    for id in &already_fact {
        println!("{}: already {}d (no change)", id, verb);
    }

    if changes_syntax.is_empty() && changes_fact.is_empty() {
        return 0;
    }

    for id in &changes_syntax {
        if dry_run {
            println!("[dry-run] would {} {}", verb, id);
        } else {
            println!("{}d {}", verb, id);
        }
    }
    for id in &changes_fact {
        if dry_run {
            println!("[dry-run] would {} {}", verb, id);
        } else {
            println!("{}d {}", verb, id);
        }
    }

    if dry_run {
        return 0;
    }

    // Load or create the project config as a toml_edit document
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = content.parse().unwrap_or_default();

    // Ensure [analyze] is an implicit table (no standalone [analyze] header)
    if !doc.contains_key("analyze") {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        doc["analyze"] = toml_edit::Item::Table(t);
    }

    // Apply syntax rule changes → [analyze.rules."id"]
    if !changes_syntax.is_empty() {
        let analyze = doc["analyze"].as_table_mut().unwrap();
        if !analyze.contains_key("rules") {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            analyze["rules"] = toml_edit::Item::Table(t);
        }
        let rules_table = analyze["rules"].as_table_mut().unwrap();
        for id in &changes_syntax {
            if !rules_table.contains_key(id) {
                rules_table[id] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            rules_table[id]["enabled"] = toml_edit::value(enable);
        }
    }

    // Apply fact rule changes → [analyze.facts-rules."id"]
    if !changes_fact.is_empty() {
        let analyze = doc["analyze"].as_table_mut().unwrap();
        if !analyze.contains_key("facts-rules") {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            analyze["facts-rules"] = toml_edit::Item::Table(t);
        }
        let facts_table = analyze["facts-rules"].as_table_mut().unwrap();
        for id in &changes_fact {
            if !facts_table.contains_key(id) {
                facts_table[id] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            facts_table[id]["enabled"] = toml_edit::value(enable);
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Failed to create config directory: {}", e);
        return 1;
    }

    if let Err(e) = std::fs::write(&config_path, doc.to_string()) {
        eprintln!("Failed to write config: {}", e);
        return 1;
    }

    0
}

// =============================================================================
// Show
// =============================================================================

fn cmd_show(
    root: &Path,
    id: &str,
    json: bool,
    use_colors: bool,
    config: &crate::config::NormalizeConfig,
) -> i32 {
    // Search syntax rules first, then fact rules
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.analyze.rules);
    let fact_rules = interpret::load_all_rules(root, &config.analyze.facts_rules);

    // Find by ID
    let found_syntax = syntax_rules.iter().find(|r| r.id == id);
    let found_fact = fact_rules.iter().find(|r| r.id == id);

    if json {
        match (found_syntax, found_fact) {
            (Some(r), _) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "id": r.id,
                        "type": "syntax",
                        "severity": r.severity.to_string(),
                        "message": r.message,
                        "tags": r.tags,
                        "languages": r.languages,
                        "allow": r.allow.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
                        "enabled": r.enabled,
                        "builtin": r.builtin,
                        "fix": r.fix,
                        "doc": r.doc,
                    }))
                    .unwrap()
                );
            }
            (_, Some(r)) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "id": r.id,
                        "type": "fact",
                        "severity": r.severity.to_string(),
                        "message": r.message,
                        "tags": r.tags,
                        "allow": r.allow.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
                        "enabled": r.enabled,
                        "builtin": r.builtin,
                        "doc": r.doc,
                    }))
                    .unwrap()
                );
            }
            _ => {
                eprintln!("Rule not found: {}", id);
                return 1;
            }
        }
        return 0;
    }

    match (found_syntax, found_fact) {
        (Some(r), _) => {
            println!("{} [syntax]", r.id);
            println!(
                "  severity: {}",
                paint_severity(&r.severity.to_string(), use_colors)
            );
            println!("  enabled:  {}", r.enabled);
            if !r.tags.is_empty() {
                println!("  tags:     {}", paint_tags(&r.tags, use_colors));
            }
            if !r.languages.is_empty() {
                println!("  langs:    {}", r.languages.join(", "));
            }
            if !r.allow.is_empty() {
                println!(
                    "  allow:    {}",
                    r.allow
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join("  ")
                );
            }
            if let Some(ref fix) = r.fix {
                if fix.is_empty() {
                    println!("  fix:      (delete match)");
                } else {
                    println!("  fix:      {}", fix);
                }
            }
            println!("  message:  {}", r.message);
            if let Some(ref doc) = r.doc {
                println!();
                println!("{}", doc);
            } else {
                println!();
                println!("(no documentation — add a markdown comment block after the frontmatter)");
            }
        }
        (_, Some(r)) => {
            println!("{} [fact]", r.id);
            println!(
                "  severity: {}",
                paint_severity(&r.severity.to_string(), use_colors)
            );
            println!("  enabled:  {}", r.enabled);
            if !r.tags.is_empty() {
                println!("  tags:     {}", paint_tags(&r.tags, use_colors));
            }
            if !r.allow.is_empty() {
                println!(
                    "  allow:    {}",
                    r.allow
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join("  ")
                );
            }
            println!("  message:  {}", r.message);
            if let Some(ref doc) = r.doc {
                println!();
                println!("{}", doc);
            } else {
                println!();
                println!("(no documentation — add a markdown comment block after the frontmatter)");
            }
        }
        _ => {
            eprintln!("Rule not found: {}", id);
            return 1;
        }
    }

    0
}

// =============================================================================
// Tags
// =============================================================================

fn cmd_tags(
    root: &Path,
    show_rules: bool,
    tag_filter: Option<&str>,
    json: bool,
    use_colors: bool,
    config: &crate::config::NormalizeConfig,
) -> i32 {
    // Collect all rules from both tiers
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.analyze.rules);
    let fact_rules = interpret::load_all_rules(root, &config.analyze.facts_rules);

    // Build the unified list for expansion
    let all_unified: Vec<UnifiedRule> = syntax_rules
        .iter()
        .map(|r| UnifiedRule {
            id: r.id.clone(),
            rule_type: "syntax",
            severity: r.severity.to_string(),
            source: if r.builtin { "builtin" } else { "project" },
            message: r.message.clone(),
            enabled: r.enabled,
            tags: r.tags.clone(),
        })
        .chain(fact_rules.iter().map(|r| UnifiedRule {
            id: r.id.clone(),
            rule_type: "fact",
            severity: r.severity.to_string(),
            source: if r.builtin { "builtin" } else { "project" },
            message: r.message.clone(),
            enabled: r.enabled,
            tags: r.tags.clone(),
        }))
        .collect();

    // tag → (origin, rule IDs)
    // "origin" values: "builtin" | "user" | "user-defined"
    let mut tag_map: std::collections::BTreeMap<String, (String, Vec<String>)> =
        std::collections::BTreeMap::new();

    // 1. Per-rule builtin/config tags
    for r in &syntax_rules {
        for tag in &r.tags {
            tag_map
                .entry(tag.clone())
                .or_insert_with(|| ("builtin".to_string(), Vec::new()))
                .1
                .push(r.id.clone());
        }
    }
    for r in &fact_rules {
        for tag in &r.tags {
            tag_map
                .entry(tag.clone())
                .or_insert_with(|| ("builtin".to_string(), Vec::new()))
                .1
                .push(r.id.clone());
        }
    }

    // 2. User-defined tag groups from [rule-tags]
    let rule_tags = &config.rule_tags.0;
    for tag_name in rule_tags.keys() {
        let entry = tag_map
            .entry(tag_name.clone())
            .or_insert_with(|| ("user-defined".to_string(), Vec::new()));
        // If already exists as a builtin tag, mark as extended
        if entry.0 == "builtin" {
            entry.0 = "builtin+user".to_string();
        }
        // Expand to get the full resolved ID set
        let mut visited = HashSet::new();
        let resolved = expand_tag(tag_name, rule_tags, &all_unified, &mut visited);
        for id in resolved {
            if !entry.1.contains(&id.to_string()) {
                entry.1.push(id.to_string());
            }
        }
    }

    // Apply tag filter
    if let Some(t) = tag_filter {
        tag_map.retain(|k, _| k == t);
    }

    if json {
        let out: Vec<_> = tag_map
            .iter()
            .map(|(tag, (origin, ids))| {
                serde_json::json!({
                    "tag": tag,
                    "origin": origin,
                    "rules": ids,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return 0;
    }

    if tag_map.is_empty() {
        println!("No tags found.");
        return 0;
    }

    for (tag, (origin, ids)) in &tag_map {
        let count = ids.len();
        let tag_display = if use_colors {
            tag_color(tag).paint(tag.as_str()).to_string()
        } else {
            tag.clone()
        };
        if show_rules {
            let ids_str: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            println!("{:20} [{}]  {}", tag_display, origin, ids_str.join("  "));
        } else {
            println!(
                "{:20} [{}]  {} rule{}",
                tag_display,
                origin,
                count,
                if count == 1 { "" } else { "s" }
            );
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
    filter_tag: Option<&str>,
    fix: bool,
    sarif: bool,
    type_filter: &RuleType,
    debug: &[String],
    json: bool,
    config: &crate::config::NormalizeConfig,
) -> i32 {
    let mut exit_code = 0;

    // If the tag is user-defined, expand it to a concrete set of rule IDs so that
    // both the syntax and fact runners can filter against it.
    let rule_tags = &config.rule_tags.0;
    let filter_ids: Option<HashSet<String>> = filter_tag.and_then(|tag| {
        if rule_tags.contains_key(tag) {
            // Build unified list for expansion
            let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.analyze.rules);
            let fact_rules = interpret::load_all_rules(root, &config.analyze.facts_rules);
            let all_unified: Vec<UnifiedRule> = syntax_rules
                .iter()
                .map(|r| UnifiedRule {
                    id: r.id.clone(),
                    rule_type: "syntax",
                    severity: r.severity.to_string(),
                    source: if r.builtin { "builtin" } else { "project" },
                    message: r.message.clone(),
                    enabled: r.enabled,
                    tags: r.tags.clone(),
                })
                .chain(fact_rules.iter().map(|r| UnifiedRule {
                    id: r.id.clone(),
                    rule_type: "fact",
                    severity: r.severity.to_string(),
                    source: if r.builtin { "builtin" } else { "project" },
                    message: r.message.clone(),
                    enabled: r.enabled,
                    tags: r.tags.clone(),
                }))
                .collect();
            let mut visited = HashSet::new();
            let ids = expand_tag(tag, rule_tags, &all_unified, &mut visited);
            Some(ids.iter().map(|s| s.to_string()).collect())
        } else {
            None // Builtin tag: handled by filter_tag pass-through
        }
    });
    // When using expanded IDs, don't also pass filter_tag (would AND incorrectly)
    let effective_tag = if filter_ids.is_some() {
        None
    } else {
        filter_tag
    };

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
            effective_tag,
            filter_ids.as_ref(),
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

    // Run fact rules (pass filter_ids for tag-based filtering)
    if matches!(type_filter, RuleType::All | RuleType::Fact) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let code = rt.block_on(run_fact_rules(
            root,
            None,
            false,
            json,
            &config.analyze.facts_rules,
            filter_ids.as_ref(),
        ));
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
    filter_ids: Option<&HashSet<String>>,
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

    // Filter to enabled rules for execution, with optional ID allowlist
    let all_rules: Vec<_> = all_rules_unfiltered
        .into_iter()
        .filter(|r| r.enabled)
        .filter(|r| filter_ids.is_none_or(|ids| ids.contains(&r.id)))
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
