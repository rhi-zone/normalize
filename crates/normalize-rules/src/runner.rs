//! Unified rule management - list, run, add, update, remove rules (syntax + fact).

use normalize_facts_rules_interpret as interpret;
pub use normalize_rules_config::{RuleOverride, RulesConfig, SarifTool};
use normalize_syntax_rules::{self, DebugFlags};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Rule type filter for list/run commands.
#[derive(Clone, Debug, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleType {
    #[default]
    All,
    Syntax,
    Fact,
    /// Native checks: stale-summary, missing-summary, check-refs, stale-docs, check-examples.
    Native,
    /// Run external tools that emit SARIF 2.1.0 output (configured via `[[rules.sarif-tools]]`).
    Sarif,
}

impl std::fmt::Display for RuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::Syntax => f.write_str("syntax"),
            Self::Fact => f.write_str("fact"),
            Self::Native => f.write_str("native"),
            Self::Sarif => f.write_str("sarif"),
        }
    }
}

impl std::str::FromStr for RuleType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "syntax" => Ok(Self::Syntax),
            "fact" => Ok(Self::Fact),
            "native" => Ok(Self::Native),
            "sarif" => Ok(Self::Sarif),
            _ => Err(format!(
                "unknown rule type: {s}; valid: all, syntax, fact, native, sarif"
            )),
        }
    }
}

/// Lock file entry tracking an imported rule
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuleLockEntry {
    source: String,
    content_hash: String,
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

/// Configuration needed by the rules runner (extracted from NormalizeConfig).
#[derive(Clone, Debug)]
pub struct RulesRunConfig {
    /// User-defined rule tag groups (`[rule-tags]` section).
    pub rule_tags: HashMap<String, Vec<String>>,
    /// Rules configuration covering all engines (syntax, fact, native, sarif).
    /// Per-rule overrides, global-allow patterns, and sarif-tools all live here.
    pub rules: RulesConfig,
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
    // Curated palette for consistent perceived lightness (OKLCH L≈0.65).
    // Red / Yellow omitted — reserved for error / warning severity.
    // All Fixed() values chosen from the 256-color cube at medium-high brightness
    // so hue varies but lightness stays consistent across terminals.
    const PALETTE: &[Color] = &[
        Color::Cyan,       // #00ffff — bright cyan
        Color::Green,      // bright green
        Color::Magenta,    // bright magenta
        Color::Fixed(80),  // #5fd7d7 — teal (brighter than Fixed(37))
        Color::Fixed(111), // #87afff — cornflower (brighter than Fixed(67))
        Color::Fixed(141), // #af87ff — light purple
        Color::Fixed(78),  // #5fd787 — spring green (brighter than Fixed(107))
        Color::Fixed(117), // #87d7ff — sky blue (brighter than Fixed(73))
        Color::Fixed(183), // #d7afff — lavender
        Color::Fixed(159), // #afffd7 — mint
    ];
    // FNV-1a hash for a stable, fast, dependency-free result
    let mut hash = 0xcbf29ce484222325u64;
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
    // Trim before matching so pre-padded strings (e.g. "warning ") still match.
    // The full `severity` string (including padding) is wrapped in the color code
    // so the visible width is preserved for column alignment.
    match severity.trim() {
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
// Rules list report
// =============================================================================

/// A single rule entry in a list report.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct RuleEntry {
    pub id: String,
    pub rule_type: String,
    pub severity: String,
    pub source: String,
    pub message: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub recommended: bool,
}

/// Report returned by `normalize rules list`.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct RulesListReport {
    pub rules: Vec<RuleEntry>,
    pub total: usize,
    pub syntax_count: usize,
    pub fact_count: usize,
    pub native_count: usize,
    pub disabled_count: usize,
}

impl normalize_output::OutputFormatter for RulesListReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        if self.rules.is_empty() {
            return "No rules found.\n".to_string();
        }
        let breakdown = {
            let mut parts = Vec::new();
            if self.syntax_count > 0 {
                parts.push(format!("{} syntax", self.syntax_count));
            }
            if self.fact_count > 0 {
                parts.push(format!("{} fact", self.fact_count));
            }
            if self.native_count > 0 {
                parts.push(format!("{} native", self.native_count));
            }
            parts.join(", ")
        };
        if self.disabled_count > 0 {
            out.push_str(&format!(
                "{} rules ({}) — {} disabled\n\n",
                self.total, breakdown, self.disabled_count
            ));
        } else {
            out.push_str(&format!("{} rules ({})\n\n", self.total, breakdown));
        }
        for r in &self.rules {
            let type_col = format!("{:<8}", format!("[{}]", r.rule_type));
            let sev_col = format!("{:<8}", r.severity);
            let state_col = if r.enabled { "   " } else { "off" };
            let tags_str = if r.tags.is_empty() {
                String::new()
            } else {
                format!(
                    "  {}",
                    r.tags
                        .iter()
                        .map(|t| format!("[{t}]"))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            out.push_str(&format!(
                "  {}  {:<30}  {}  {}  {:<7}{}\n",
                type_col, r.id, sev_col, state_col, r.source, tags_str
            ));
            out.push_str(&format!("            {}\n", r.message));
        }
        out.push_str("\nConfigure: [rules.\"<id>\"] in .normalize/config.toml\n");
        out.push_str("  severity, enabled, allow — or: normalize rules enable/disable <id>\n");
        out.push_str("  Global patterns: [rules] global-allow = [\"**/fixtures/**\"]\n");
        out.push_str("  Custom tag groups: [rule-tags] my-group = [\"tag1\", \"tag2\"]\n");
        out
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::{Color, Style};

        let mut out = String::new();
        if self.rules.is_empty() {
            return "No rules found.\n".to_string();
        }
        let breakdown = {
            let mut parts = Vec::new();
            if self.syntax_count > 0 {
                parts.push(format!("{} syntax", self.syntax_count));
            }
            if self.fact_count > 0 {
                parts.push(format!("{} fact", self.fact_count));
            }
            if self.native_count > 0 {
                parts.push(format!("{} native", self.native_count));
            }
            parts.join(", ")
        };
        let header = if self.disabled_count > 0 {
            format!(
                "{} rules ({}) — {} disabled",
                Color::White.bold().paint(self.total.to_string()),
                breakdown,
                Color::DarkGray.paint(self.disabled_count.to_string())
            )
        } else {
            format!(
                "{} rules ({})",
                Color::White.bold().paint(self.total.to_string()),
                breakdown
            )
        };
        out.push_str(&format!("{header}\n\n"));

        // Column headers
        let gray = Color::DarkGray;
        out.push_str(&format!(
            "{}\n",
            gray.paint(format!(
                "  {:<6}  {:<30}  {:<8}  ST  {:<7}  TAGS",
                "TYPE", "ID", "SEVERITY", "SOURCE"
            ))
        ));

        for r in &self.rules {
            let type_col = paint_rule_type(&r.rule_type);
            let sev_col = paint_severity(&format!("{:<8}", r.severity), true);
            let state_col = if r.enabled {
                Style::new().paint(" ● ").to_string()
            } else {
                Color::DarkGray.paint(" ○ ").to_string()
            };
            let tags_str = if r.tags.is_empty() {
                String::new()
            } else {
                format!("  {}", paint_tags(&r.tags, true))
            };
            // Pad plain text BEFORE colorizing to preserve column widths
            let id_padded = format!("{:<30}", r.id);
            let id_col = if r.enabled {
                id_padded
            } else {
                Color::DarkGray.paint(id_padded).to_string()
            };
            out.push_str(&format!(
                "  {type_col}  {id_col}  {sev_col}  {state_col}  {:<7}{tags_str}\n",
                r.source
            ));
            let desc = if r.enabled {
                Color::DarkGray.paint(&r.message).to_string()
            } else {
                Color::DarkGray.dimmed().paint(&r.message).to_string()
            };
            out.push_str(&format!("            {desc}\n"));
        }
        let dim = Color::DarkGray;
        out.push('\n');
        out.push_str(
            &dim.paint("Configure: [rules.\"<id>\"] in .normalize/config.toml\n")
                .to_string(),
        );
        out.push_str(
            &dim.paint("  severity, enabled, allow — or: normalize rules enable/disable <id>\n")
                .to_string(),
        );
        out.push_str(
            &dim.paint("  Global patterns: [rules] global-allow = [\"**/fixtures/**\"]\n")
                .to_string(),
        );
        out.push_str(
            &dim.paint("  Custom tag groups: [rule-tags] my-group = [\"tag1\", \"tag2\"]\n")
                .to_string(),
        );
        out
    }
}

/// Color the rule type indicator: `fact` in blue, `syntax` in cyan.
/// Pad the plain text FIRST so ANSI codes don't corrupt column widths.
fn paint_rule_type(rule_type: &str) -> String {
    use nu_ansi_term::Color;
    let col = match rule_type {
        "syntax" => Color::Cyan,
        "fact" => Color::Blue,
        "native" => Color::Green,
        _ => Color::DarkGray,
    };
    // Pad plain text to 6 chars, then wrap in color so visible width is preserved
    col.paint(format!("{:<6}", rule_type)).to_string()
}

/// Unified rule descriptor for display (private).
struct UnifiedRule {
    id: String,
    rule_type: &'static str,
    severity: String,
    source: &'static str,
    message: String,
    enabled: bool,
    tags: Vec<String>,
    recommended: bool,
}

/// Filters applied when listing rules via [`build_list_report`].
pub struct ListFilters<'a> {
    /// Restrict to a specific rule engine type (syntax, fact, native, all, …).
    pub type_filter: &'a RuleType,
    /// If `Some`, only include rules whose tags contain this value.
    pub tag: Option<&'a str>,
    /// If `true`, only include rules that are currently enabled.
    /// `enabled` and `disabled` are mutually exclusive — setting both returns no rules.
    pub enabled: bool,
    /// If `true`, only include rules that are currently disabled.
    /// `enabled` and `disabled` are mutually exclusive — setting both returns no rules.
    pub disabled: bool,
}

/// Build a `RulesListReport` from the index, applying the given filters.
pub fn build_list_report(
    root: &Path,
    filters: &ListFilters<'_>,
    config: &RulesRunConfig,
) -> RulesListReport {
    let mut all_rules: Vec<UnifiedRule> = Vec::new();

    // Load syntax rules
    if matches!(filters.type_filter, RuleType::All | RuleType::Syntax) {
        let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.rules);
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
                recommended: r.recommended,
            });
        }
    }

    // Load fact rules
    if matches!(filters.type_filter, RuleType::All | RuleType::Fact) {
        let fact_rules = interpret::load_all_rules(root, &config.rules);
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
                recommended: r.recommended,
            });
        }
    }

    // Load native rules (static descriptors with config overrides applied)
    if matches!(filters.type_filter, RuleType::All | RuleType::Native) {
        for desc in normalize_native_rules::NATIVE_RULES {
            let override_ = config.rules.rules.get(desc.id);
            let severity = override_
                .and_then(|o| o.severity.as_deref())
                .unwrap_or(desc.default_severity)
                .to_string();
            let enabled = override_.and_then(|o| o.enabled).unwrap_or(true);
            let mut tags: Vec<String> = desc.tags.iter().map(|t| t.to_string()).collect();
            if let Some(o) = override_ {
                tags.extend(o.tags.iter().cloned());
            }
            all_rules.push(UnifiedRule {
                id: desc.id.to_string(),
                rule_type: "native",
                severity,
                source: "builtin",
                message: desc.message.to_string(),
                enabled,
                tags,
                recommended: false,
            });
        }
    }

    // Apply filters (all compose via AND)
    if let Some(tag) = filters.tag {
        let rule_tags = &config.rule_tags;
        let mut visited = HashSet::new();
        let matching_ids: HashSet<String> = expand_tag(tag, rule_tags, &all_rules, &mut visited)
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        all_rules.retain(|r| matching_ids.contains(&r.id));
    }
    if filters.enabled {
        all_rules.retain(|r| r.enabled);
    }
    if filters.disabled {
        all_rules.retain(|r| !r.enabled);
    }

    // Sort by type then id for stable output
    all_rules.sort_by(|a, b| a.rule_type.cmp(b.rule_type).then(a.id.cmp(&b.id)));

    let syntax_count = all_rules.iter().filter(|r| r.rule_type == "syntax").count();
    let fact_count = all_rules.iter().filter(|r| r.rule_type == "fact").count();
    let native_count = all_rules.iter().filter(|r| r.rule_type == "native").count();
    let disabled_count = all_rules.iter().filter(|r| !r.enabled).count();
    let total = all_rules.len();

    let rules = all_rules
        .into_iter()
        .map(|r| RuleEntry {
            id: r.id,
            rule_type: r.rule_type.to_string(),
            severity: r.severity,
            source: r.source.to_string(),
            message: r.message,
            enabled: r.enabled,
            tags: r.tags,
            recommended: r.recommended,
        })
        .collect();

    RulesListReport {
        rules,
        total,
        syntax_count,
        fact_count,
        native_count,
        disabled_count,
    }
}

// =============================================================================
// Enable / Disable
// =============================================================================

fn build_unified_rules(
    syntax_rules: &[normalize_syntax_rules::Rule],
    fact_rules: &[interpret::FactsRule],
) -> Vec<UnifiedRule> {
    syntax_rules
        .iter()
        .map(|r| UnifiedRule {
            id: r.id.clone(),
            rule_type: "syntax",
            severity: r.severity.to_string(),
            source: if r.builtin { "builtin" } else { "project" },
            message: r.message.clone(),
            enabled: r.enabled,
            tags: r.tags.clone(),
            recommended: r.recommended,
        })
        .chain(fact_rules.iter().map(|r| UnifiedRule {
            id: r.id.clone(),
            rule_type: "fact",
            severity: r.severity.to_string(),
            source: if r.builtin { "builtin" } else { "project" },
            message: r.message.clone(),
            enabled: r.enabled,
            tags: r.tags.clone(),
            recommended: r.recommended,
        }))
        .collect()
}

pub fn enable_disable(
    root: &Path,
    id_or_tag: &str,
    enable: bool,
    dry_run: bool,
    config: &RulesRunConfig,
) -> Result<String, String> {
    // Resolve which rule IDs to affect
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.rules);
    let fact_rules = interpret::load_all_rules(root, &config.rules);
    let all_unified = build_unified_rules(&syntax_rules, &fact_rules);

    // Exact ID match takes priority; otherwise expand as tag (includes user-defined groups)
    let rule_tags = &config.rule_tags;
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
        return Err(format!(
            "No rules found matching '{}' (not a rule ID or tag)",
            id_or_tag
        ));
    }

    let verb = if enable { "enable" } else { "disable" };
    let config_path = root.join(".normalize").join("config.toml");

    // Collect all changes to apply: (section, rule_id)
    // syntax rules → [rules."id"]
    // fact rules   → [rules."id"] (unified config section)
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

    let mut out = String::new();

    for id in &already_syntax {
        out.push_str(&format!("{}: already {}d (no change)\n", id, verb));
    }
    for id in &already_fact {
        out.push_str(&format!("{}: already {}d (no change)\n", id, verb));
    }

    if changes_syntax.is_empty() && changes_fact.is_empty() {
        return Ok(out);
    }

    for id in &changes_syntax {
        if dry_run {
            out.push_str(&format!("[dry-run] would {} {}\n", verb, id));
        } else {
            out.push_str(&format!("{}d {}\n", verb, id));
        }
    }
    for id in &changes_fact {
        if dry_run {
            out.push_str(&format!("[dry-run] would {} {}\n", verb, id));
        } else {
            out.push_str(&format!("{}d {}\n", verb, id));
        }
    }

    if dry_run {
        return Ok(out);
    }

    // Load or create the project config as a toml_edit document
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = content.parse().unwrap_or_else(|e| {
        tracing::warn!("failed to parse existing config, using defaults: {}", e);
        toml_edit::DocumentMut::default()
    });

    // Ensure [rules] exists as an explicit table section.
    // If the existing entry is an inline table (e.g. `rules = {}`), bail out with an
    // actionable error — converting inline tables in-place is lossy and surprising.
    if !doc.contains_key("rules") {
        let mut t = toml_edit::Table::new();
        t.set_implicit(true);
        doc["rules"] = toml_edit::Item::Table(t);
    } else if doc["rules"].is_inline_table() {
        return Err("Cannot update rules config: the existing 'rules' entry in \
             .normalize/config.toml is an inline table (e.g. `rules = {...}`). \
             Convert it to a [rules] section first."
            .to_string());
    }

    // Apply syntax rule changes → [rules."id"]
    if !changes_syntax.is_empty() {
        let rules_table = doc["rules"]
            .as_table_mut()
            .ok_or_else(|| "'rules' is not a TOML table".to_string())?;
        for id in &changes_syntax {
            if !rules_table.contains_key(id) {
                rules_table[id] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            rules_table[id]["enabled"] = toml_edit::value(enable);
        }
    }

    // Apply fact rule changes → [rules."id"] (same section as syntax rules)
    if !changes_fact.is_empty() {
        let rules_table = doc["rules"]
            .as_table_mut()
            .ok_or_else(|| "'rules' is not a TOML table".to_string())?;
        for id in &changes_fact {
            if !rules_table.contains_key(id) {
                rules_table[id] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            rules_table[id]["enabled"] = toml_edit::value(enable);
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {e}"))?;
    }

    std::fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("Failed to write config: {e}"))?;

    Ok(out)
}

// =============================================================================
// Show
// =============================================================================

pub fn show_rule(
    root: &Path,
    id: &str,
    use_colors: bool,
    config: &RulesRunConfig,
) -> Result<String, String> {
    // Search syntax rules first, then fact rules
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.rules);
    let fact_rules = interpret::load_all_rules(root, &config.rules);

    // Find by ID
    let found_syntax = syntax_rules.iter().find(|r| r.id == id);
    let found_fact = fact_rules.iter().find(|r| r.id == id);

    let mut out = String::new();

    match (found_syntax, found_fact) {
        (Some(r), _) => {
            out.push_str(&format!("{} [syntax]\n", r.id));
            out.push_str(&format!(
                "  severity: {}\n",
                paint_severity(&r.severity.to_string(), use_colors)
            ));
            out.push_str(&format!("  enabled:  {}\n", r.enabled));
            if !r.tags.is_empty() {
                out.push_str(&format!(
                    "  tags:     {}\n",
                    paint_tags(&r.tags, use_colors)
                ));
            }
            if !r.languages.is_empty() {
                out.push_str(&format!("  langs:    {}\n", r.languages.join(", ")));
            }
            if !r.allow.is_empty() {
                out.push_str(&format!(
                    "  allow:    {}\n",
                    r.allow
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join("  ")
                ));
            }
            if let Some(ref fix) = r.fix {
                if fix.is_empty() {
                    out.push_str("  fix:      (delete match)\n");
                } else {
                    out.push_str(&format!("  fix:      {}\n", fix));
                }
            }
            out.push_str(&format!("  message:  {}\n", r.message));
            if let Some(ref doc) = r.doc {
                out.push('\n');
                out.push_str(doc);
                out.push('\n');
            } else {
                out.push('\n');
                out.push_str(
                    "(no documentation — add a markdown comment block after the frontmatter)\n",
                );
            }
            out.push('\n');
            out.push_str(&format_config_snippet(&r.id, config.rules.rules.get(&r.id)));
        }
        (_, Some(r)) => {
            out.push_str(&format!("{} [fact]\n", r.id));
            out.push_str(&format!(
                "  severity: {}\n",
                paint_severity(&r.severity.to_string(), use_colors)
            ));
            out.push_str(&format!("  enabled:  {}\n", r.enabled));
            if !r.tags.is_empty() {
                out.push_str(&format!(
                    "  tags:     {}\n",
                    paint_tags(&r.tags, use_colors)
                ));
            }
            if !r.allow.is_empty() {
                out.push_str(&format!(
                    "  allow:    {}\n",
                    r.allow
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join("  ")
                ));
            }
            out.push_str(&format!("  message:  {}\n", r.message));
            if let Some(ref doc) = r.doc {
                out.push('\n');
                out.push_str(doc);
                out.push('\n');
            } else {
                out.push('\n');
                out.push_str(
                    "(no documentation — add a markdown comment block after the frontmatter)\n",
                );
            }
            out.push('\n');
            out.push_str(&format_config_snippet(&r.id, config.rules.rules.get(&r.id)));
        }
        _ => return Err(format!("Rule not found: {}", id)),
    }

    Ok(out)
}

fn format_config_snippet(
    id: &str,
    override_: Option<&normalize_rules_config::RuleOverride>,
) -> String {
    let mut out = String::new();
    out.push_str("Configuration (.normalize/config.toml):\n");
    if let Some(o) = override_ {
        out.push_str(&format!("  [rules.\"{id}\"]\n"));
        if let Some(ref sev) = o.severity {
            out.push_str(&format!("  severity = \"{sev}\"\n"));
        }
        if let Some(enabled) = o.enabled {
            out.push_str(&format!("  enabled = {enabled}\n"));
        }
        if !o.allow.is_empty() {
            let patterns = o
                .allow
                .iter()
                .map(|p| format!("\"{p}\""))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  allow = [{patterns}]\n"));
        }
    } else {
        out.push_str("  # No overrides set. Example:\n");
        out.push_str(&format!("  [rules.\"{id}\"]\n"));
        out.push_str("  severity = \"error\"          # error | warning | info | hint\n");
        out.push_str("  enabled = false              # disable this rule\n");
        out.push_str("  allow = [\"**/tests/**\"]      # skip matching files\n");
    }
    out.push('\n');
    out.push_str(&format!("  # Or use: normalize rules enable {id}\n"));
    out.push_str(&format!("  #         normalize rules disable {id}\n"));
    out
}

// =============================================================================
// Tags
// =============================================================================

pub fn list_tags(
    root: &Path,
    show_rules: bool,
    tag_filter: Option<&str>,
    use_colors: bool,
    config: &RulesRunConfig,
) -> Result<String, String> {
    // Collect all rules from both tiers
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.rules);
    let fact_rules = interpret::load_all_rules(root, &config.rules);

    // Build the unified list for expansion
    let all_unified = build_unified_rules(&syntax_rules, &fact_rules);

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
    let rule_tags = &config.rule_tags;
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

    let mut out = String::new();

    if tag_map.is_empty() {
        out.push_str("No tags found.\n");
        return Ok(out);
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
            out.push_str(&format!(
                "{:20} [{}]  {}\n",
                tag_display,
                origin,
                ids_str.join("  ")
            ));
        } else {
            out.push_str(&format!(
                "{:20} [{}]  {} rule{}\n",
                tag_display,
                origin,
                count,
                if count == 1 { "" } else { "s" }
            ));
        }
    }

    Ok(out)
}

/// Collect fact rule diagnostics without printing (returns raw diagnostics).
pub async fn collect_fact_diagnostics(
    root: &Path,
    config: &RulesConfig,
    filter_ids: Option<&HashSet<String>>,
    filter_rule: Option<&str>,
) -> Vec<normalize_facts_rules_api::Diagnostic> {
    let all_rules_unfiltered = interpret::load_all_rules(root, config);
    let all_rules: Vec<_> = all_rules_unfiltered
        .into_iter()
        .filter(|r| r.enabled)
        .filter(|r| filter_ids.is_none_or(|ids| ids.contains(&r.id)))
        .filter(|r| filter_rule.is_none_or(|id| r.id == id))
        .collect();

    if all_rules.is_empty() {
        return Vec::new();
    }

    let relations = match ensure_relations(root).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error building relations: {}", e);
            return Vec::new();
        }
    };

    let rule_refs: Vec<&interpret::FactsRule> = all_rules.iter().collect();
    let mut all_diagnostics = match interpret::run_rules_batch(&rule_refs, &relations) {
        Ok(diagnostics) => diagnostics,
        Err(e) => {
            eprintln!("Error running fact rules batch: {}", e);
            Vec::new()
        }
    };

    interpret::filter_inline_allowed(&mut all_diagnostics, root);
    all_diagnostics
}

/// Apply `RulesConfig` severity/enabled overrides to issues in a `DiagnosticsReport`.
/// This lets native checks (stale-summary, missing-summary, check-refs, etc.) be
/// configured via `[rules."rule-id"]` in normalize.toml, just like syntax rules.
pub fn apply_native_rules_config(
    report: &mut normalize_output::diagnostics::DiagnosticsReport,
    config: &RulesConfig,
) {
    use normalize_output::diagnostics::Severity;
    report.issues.retain_mut(|issue| {
        let Some(override_) = config.rules.get(&issue.rule_id) else {
            return true;
        };
        // enabled=false suppresses the issue entirely
        if override_.enabled == Some(false) {
            return false;
        }
        // allow patterns suppress matching issues
        if !override_.allow.is_empty() {
            let patterns: Vec<glob::Pattern> = override_
                .allow
                .iter()
                .filter_map(|p| glob::Pattern::new(p).ok())
                .collect();
            if patterns.iter().any(|p| p.matches(&issue.file)) {
                return false;
            }
        }
        if let Some(sev_str) = &override_.severity {
            issue.severity = match sev_str.as_str() {
                "error" => Severity::Error,
                "warning" => Severity::Warning,
                "info" => Severity::Info,
                "hint" => Severity::Hint,
                _ => issue.severity,
            };
        }
        true
    });
}

/// Run all rules (syntax + fact) and return a unified DiagnosticsReport.
pub fn run_rules_report(
    root: &Path,
    project_root: &Path,
    filter_rule: Option<&str>,
    filter_tag: Option<&str>,
    engine: &RuleType,
    debug: &[String],
    config: &RulesRunConfig,
) -> normalize_output::diagnostics::DiagnosticsReport {
    use normalize_output::diagnostics::DiagnosticsReport;

    let mut report = DiagnosticsReport::new();

    // Expand user-defined tags to concrete rule IDs
    let rule_tags = &config.rule_tags;
    let filter_ids: Option<HashSet<String>> = filter_tag.and_then(|tag| {
        if rule_tags.contains_key(tag) {
            let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.rules);
            let fact_rules = interpret::load_all_rules(root, &config.rules);
            let all_unified = build_unified_rules(&syntax_rules, &fact_rules);
            let mut visited = HashSet::new();
            let ids = expand_tag(tag, rule_tags, &all_unified, &mut visited);
            Some(ids.iter().map(|s| s.to_string()).collect())
        } else {
            None
        }
    });
    let effective_tag = if filter_ids.is_some() {
        None
    } else {
        filter_tag
    };

    // Syntax rules
    if matches!(engine, RuleType::All | RuleType::Syntax) {
        let debug_flags = DebugFlags::from_args(debug);
        let findings = crate::cmd_rules::run_syntax_rules(
            root,
            project_root,
            filter_rule,
            effective_tag,
            filter_ids.as_ref(),
            &config.rules,
            &debug_flags,
        );
        // Count unique files with violations for the report header.
        let unique_files: HashSet<&std::path::Path> =
            findings.iter().map(|f| f.file.as_path()).collect();
        report.files_checked = report.files_checked.max(unique_files.len());
        for f in &findings {
            report.issues.push(finding_to_issue(f, root));
        }
        report.sources_run.push("syntax-rules".into());
    }

    // Fact rules
    if matches!(engine, RuleType::All | RuleType::Fact) {
        let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
            tracing::warn!("failed to create tokio runtime: {}", e);
            panic!("failed to create tokio runtime: {}", e)
        });
        let diagnostics = rt.block_on(collect_fact_diagnostics(
            project_root,
            &config.rules,
            filter_ids.as_ref(),
            filter_rule,
        ));
        // Apply global-allow patterns from [rules] to fact-rule diagnostics.
        // Syntax rules apply global_allow during load_all_rules(); fact rules need it here.
        let global_allow: Vec<glob::Pattern> = config
            .rules
            .global_allow
            .iter()
            .filter_map(|s| glob::Pattern::new(s).ok())
            .collect();
        for d in &diagnostics {
            let file = match &d.location {
                abi_stable::std_types::ROption::RSome(loc) => loc.file.as_str(),
                abi_stable::std_types::ROption::RNone => d.message.as_str(),
            };
            if global_allow.is_empty() || !global_allow.iter().any(|p| p.matches(file)) {
                report.issues.push(abi_diagnostic_to_issue(d));
            }
        }
        report.sources_run.push("fact-rules".into());
    }

    // SARIF passthrough: run external tools and merge their SARIF output
    if matches!(engine, RuleType::Sarif) {
        let sarif_report = run_sarif_tools(root, &config.rules.sarif_tools);
        report.merge(sarif_report);
    }

    report.sort();
    report
}

/// Run external SARIF tools and merge their output into a DiagnosticsReport.
/// Each tool's command is run with `{root}` replaced by the project root path.
/// Tools must emit SARIF 2.1.0 JSON to stdout.
pub fn run_sarif_tools(
    root: &Path,
    tools: &[SarifTool],
) -> normalize_output::diagnostics::DiagnosticsReport {
    use normalize_output::diagnostics::{DiagnosticsReport, Issue, Severity};

    let mut report = DiagnosticsReport::new();
    let root_str = root.to_string_lossy();

    for tool in tools {
        if tool.command.is_empty() {
            continue;
        }
        let args: Vec<String> = tool
            .command
            .iter()
            .map(|a| a.replace("{root}", &root_str))
            .collect();

        let output = std::process::Command::new(&args[0])
            .args(&args[1..])
            .current_dir(root)
            .output();

        let stdout = match output {
            Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
            Err(e) => {
                let msg = format!("failed to run: {e}");
                eprintln!("normalize: SARIF tool '{}' {}", tool.name, msg);
                report
                    .tool_errors
                    .push(normalize_output::diagnostics::ToolFailure {
                        tool: tool.name.clone(),
                        message: msg,
                    });
                continue;
            }
        };

        let sarif: serde_json::Value = match serde_json::from_str(&stdout) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("did not emit valid JSON: {e}");
                eprintln!("normalize: SARIF tool '{}' {}", tool.name, msg);
                report
                    .tool_errors
                    .push(normalize_output::diagnostics::ToolFailure {
                        tool: tool.name.clone(),
                        message: msg,
                    });
                continue;
            }
        };

        let runs = match sarif.get("runs").and_then(|v| v.as_array()) {
            Some(r) => r,
            None => {
                let msg = "output missing 'runs' array".to_string();
                eprintln!("normalize: SARIF tool '{}' {}", tool.name, msg);
                report
                    .tool_errors
                    .push(normalize_output::diagnostics::ToolFailure {
                        tool: tool.name.clone(),
                        message: msg,
                    });
                continue;
            }
        };

        for run in runs {
            let driver_name = run
                .pointer("/tool/driver/name")
                .and_then(|v| v.as_str())
                .unwrap_or(&tool.name);
            let source = format!("sarif:{}", driver_name);

            let results = run.get("results").and_then(|v| v.as_array());
            let Some(results) = results else { continue };

            for result in results {
                let rule_id = result
                    .get("ruleId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let message = result
                    .pointer("/message/text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let level = result
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("warning");
                let severity = match level {
                    "error" => Severity::Error,
                    "warning" => Severity::Warning,
                    "note" | "none" => Severity::Info,
                    _ => Severity::Warning,
                };

                // Extract location from first entry
                let loc = result.pointer("/locations/0/physicalLocation");
                let file = loc
                    .and_then(|l| l.pointer("/artifactLocation/uri"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let line = loc
                    .and_then(|l| l.pointer("/region/startLine"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let column = loc
                    .and_then(|l| l.pointer("/region/startColumn"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);

                report.issues.push(Issue {
                    file,
                    line,
                    column,
                    end_line: None,
                    end_column: None,
                    rule_id,
                    message,
                    severity,
                    source: source.clone(),
                    related: vec![],
                    suggestion: None,
                });
            }

            report.sources_run.push(source);
        }
    }

    report
}

/// Build relations from the index, auto-building the index if it doesn't exist.
async fn ensure_relations(root: &Path) -> Result<normalize_facts_rules_api::Relations, String> {
    match build_relations_from_index(root).await {
        Ok(r) => Ok(r),
        Err(_) => {
            tracing::info!("Facts index not found. Building...");
            let normalize_dir = get_normalize_dir(root);
            let db_path = normalize_dir.join("index.sqlite");
            let mut idx = normalize_facts::FileIndex::open(&db_path, root)
                .await
                .map_err(|e| format!("Failed to open index: {}", e))?;
            let count = idx
                .refresh()
                .await
                .map_err(|e| format!("Failed to index files: {}", e))?;
            tracing::info!("Indexed {} files.", count);
            let stats = idx
                .refresh_call_graph()
                .await
                .map_err(|e| format!("Failed to index call graph: {}", e))?;
            tracing::info!(
                "Indexed {} symbols, {} calls, {} imports.",
                stats.symbols,
                stats.calls,
                stats.imports
            );
            build_relations_from_index(root).await
        }
    }
}

/// Get the normalize data directory for a project.
fn get_normalize_dir(root: &Path) -> std::path::PathBuf {
    if let Ok(index_dir) = std::env::var("NORMALIZE_INDEX_DIR") {
        let path = std::path::PathBuf::from(&index_dir);
        if path.is_absolute() {
            return path;
        }
        // Relative path: use XDG_DATA_HOME/normalize/<relative>
        let data_home = std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".local/share")
            });
        return data_home.join("normalize").join(&index_dir);
    }
    root.join(".normalize")
}

/// Build Relations from the file index.
pub async fn build_relations_from_index(
    root: &Path,
) -> Result<normalize_facts_rules_api::Relations, String> {
    use normalize_facts_rules_api::Relations;

    let normalize_dir = get_normalize_dir(root);
    let db_path = normalize_dir.join("index.sqlite");
    let idx = normalize_facts::FileIndex::open(&db_path, root)
        .await
        .map_err(|e| format!("Failed to open index: {}", e))?;

    let mut relations = Relations::new();

    // Get symbols (file, name, kind, start_line, end_line, parent, visibility)
    let symbols = idx
        .all_symbols_with_details()
        .await
        .map_err(|e| format!("Failed to get symbols: {}", e))?;

    for (file, name, kind, start_line, end_line, parent, visibility, is_impl) in &symbols {
        relations.add_symbol(file, name, kind, *start_line as u32);
        relations.add_symbol_range(file, name, *start_line as u32, *end_line as u32);
        relations.add_visibility(file, name, visibility);
        if let Some(parent_name) = parent {
            relations.add_parent(file, name, parent_name);
        }
        if *is_impl {
            relations.add_is_impl(file, name);
        }
    }

    // Get symbol attributes
    let attrs = idx
        .all_symbol_attributes()
        .await
        .map_err(|e| format!("Failed to get symbol attributes: {}", e))?;

    for (file, name, attribute) in &attrs {
        relations.add_attribute(file, name, attribute);
    }

    // Get symbol implements
    let implements = idx
        .all_symbol_implements()
        .await
        .map_err(|e| format!("Failed to get symbol implements: {}", e))?;

    for (file, name, interface) in &implements {
        relations.add_implements(file, name, interface);
    }

    // Get type methods
    let type_methods = idx
        .all_type_methods()
        .await
        .map_err(|e| format!("Failed to get type methods: {}", e))?;

    for (file, type_name, method_name) in &type_methods {
        relations.add_type_method(file, type_name, method_name);
    }

    // Get imports (file, module, name, line)
    let imports = idx
        .all_imports()
        .await
        .map_err(|e| format!("Failed to get imports: {}", e))?;

    for (file, module, name, _line) in imports {
        relations.add_import(&file, &module, &name);
    }

    // Get calls with qualifiers (caller_file, caller_symbol, callee_name, qualifier, line)
    let calls = idx
        .all_calls_with_qualifiers()
        .await
        .map_err(|e| format!("Failed to get calls: {}", e))?;

    for (file, caller, callee, qualifier, line) in &calls {
        relations.add_call(file, caller, callee, *line);
        if let Some(qual) = qualifier {
            relations.add_qualifier(file, caller, callee, qual);
        }
    }

    Ok(relations)
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

pub fn add_rule(url: &str, global: bool) -> Result<(), String> {
    let rules_dir =
        rules_dir(global).ok_or_else(|| "Could not determine rules directory".to_string())?;

    std::fs::create_dir_all(&rules_dir)
        .map_err(|e| format!("Failed to create rules directory: {e}"))?;

    let content = download_url(url).map_err(|e| format!("Failed to download rule: {e}"))?;

    let rule_id = extract_rule_id(&content).ok_or_else(|| {
        "Could not extract rule ID from downloaded content. Rule must have TOML frontmatter with 'id' field".to_string()
    })?;

    let ext = detect_extension(url);
    let rule_path = rules_dir.join(format!("{}.{}", rule_id, ext));
    std::fs::write(&rule_path, &content).map_err(|e| format!("Failed to save rule: {e}"))?;

    let lock_path =
        lock_file_path(global).ok_or_else(|| "Could not determine lock file path".to_string())?;

    let mut lock = RulesLock::load(&lock_path);
    lock.rules.insert(
        rule_id.clone(),
        RuleLockEntry {
            source: url.to_string(),
            content_hash: content_hash(&content),
            added: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        },
    );

    if let Err(e) = lock.save(&lock_path) {
        eprintln!("Warning: Failed to update lock file: {}", e);
    }

    println!("Added rule '{}' from {}", rule_id, url);
    println!("Saved to: {}", rule_path.display());

    Ok(())
}

pub fn update_rules(rule_id: Option<&str>) -> Result<(), String> {
    let mut updated = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

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

    if updated.is_empty() && errors.is_empty() {
        println!("No imported rules to update.");
    } else {
        for id in &updated {
            println!("Updated: {}", id);
        }
        for (id, err) in &errors {
            eprintln!("Failed to update {}: {}", id, err);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("{} rule(s) failed to update", errors.len()))
    }
}

pub fn remove_rule(rule_id: &str) -> Result<(), String> {
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

    if removed {
        println!("Removed rule '{}'", rule_id);
        Ok(())
    } else {
        Err(format!("Rule '{}' not found in lock file", rule_id))
    }
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

fn content_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

// =============================================================================
// Diagnostic conversion helpers (from normalize's diagnostic_convert.rs)
// =============================================================================

/// Convert a syntax-rules `Finding` into a unified `Issue`.
pub fn finding_to_issue(
    f: &normalize_syntax_rules::Finding,
    root: &std::path::Path,
) -> normalize_output::diagnostics::Issue {
    use normalize_output::diagnostics::Issue;
    // If root is a file path, use its parent for strip_prefix so we get "file.rs" not "".
    let effective_root;
    let root = if root.is_file() {
        effective_root = root.parent().unwrap_or(root).to_path_buf();
        &effective_root
    } else {
        root
    };
    let rel_path = f.file.strip_prefix(root).unwrap_or(&f.file);
    Issue {
        file: rel_path.to_string_lossy().to_string(),
        line: Some(f.start_line),
        column: Some(f.start_col),
        end_line: Some(f.end_line),
        end_column: Some(f.end_col),
        rule_id: f.rule_id.clone(),
        message: f.message.clone(),
        severity: syntax_severity(f.severity),
        source: "syntax-rules".into(),
        related: Vec::new(),
        suggestion: f.fix.clone(),
    }
}

/// Convert syntax-rules `Severity` to output `Severity`.
fn syntax_severity(s: normalize_syntax_rules::Severity) -> normalize_output::diagnostics::Severity {
    use normalize_output::diagnostics::Severity;
    match s {
        normalize_syntax_rules::Severity::Error => Severity::Error,
        normalize_syntax_rules::Severity::Warning => Severity::Warning,
        normalize_syntax_rules::Severity::Info => Severity::Info,
        normalize_syntax_rules::Severity::Hint => Severity::Hint,
    }
}

/// Convert a facts-rules-api `Diagnostic` into a unified `Issue`.
pub fn abi_diagnostic_to_issue(
    d: &normalize_facts_rules_api::Diagnostic,
) -> normalize_output::diagnostics::Issue {
    use abi_stable::std_types::ROption;
    use normalize_output::diagnostics::{Issue, RelatedLocation};

    let (file, line, column) = match &d.location {
        ROption::RSome(loc) => (
            loc.file.to_string(),
            Some(loc.line as usize),
            match &loc.column {
                ROption::RSome(c) => Some(*c as usize),
                ROption::RNone => None,
            },
        ),
        ROption::RNone => (String::new(), None, None),
    };

    let related = d
        .related
        .iter()
        .map(|loc| RelatedLocation {
            file: loc.file.to_string(),
            line: Some(loc.line as usize),
            message: None,
        })
        .collect();

    let suggestion = match &d.suggestion {
        ROption::RSome(s) => Some(s.to_string()),
        ROption::RNone => None,
    };

    Issue {
        file,
        line,
        column,
        end_line: None,
        end_column: None,
        rule_id: d.rule_id.to_string(),
        message: d.message.to_string(),
        severity: abi_level(d.level),
        source: "fact-rules".into(),
        related,
        suggestion,
    }
}

/// Convert facts-rules-api `DiagnosticLevel` to output `Severity`.
fn abi_level(
    level: normalize_facts_rules_api::DiagnosticLevel,
) -> normalize_output::diagnostics::Severity {
    use normalize_output::diagnostics::Severity;
    match level {
        normalize_facts_rules_api::DiagnosticLevel::Hint => Severity::Hint,
        normalize_facts_rules_api::DiagnosticLevel::Warning => Severity::Warning,
        normalize_facts_rules_api::DiagnosticLevel::Error => Severity::Error,
    }
}

/// Format a diagnostic for terminal display.
pub fn format_diagnostic(diag: &normalize_facts_rules_api::Diagnostic, use_colors: bool) -> String {
    crate::loader::format_diagnostic(diag, use_colors)
}
