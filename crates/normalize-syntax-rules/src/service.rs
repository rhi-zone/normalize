//! Standalone CLI service for normalize-syntax-rules.
//!
//! Exposes rule management as a standalone binary:
//! - `run` — run rules against a directory
//! - `list` — list available rules

use crate::{DebugFlags, Finding, Rule, Severity, apply_fixes, load_all_rules, run_rules};
use normalize_languages::parsers::grammar_loader;
use normalize_output::OutputFormatter;
use schemars::JsonSchema;
use serde::Serialize;
use server_less::cli;
use std::path::PathBuf;

// =============================================================================
// Output types
// =============================================================================

/// A single rule finding serialized for output.
#[derive(Serialize, JsonSchema)]
pub struct FindingItem {
    pub rule_id: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub severity: String,
    pub message: String,
}

impl From<&Finding> for FindingItem {
    fn from(f: &Finding) -> Self {
        Self {
            rule_id: f.rule_id.clone(),
            file: f.file.display().to_string(),
            line: f.start_line,
            col: f.start_col,
            severity: f.severity.to_string(),
            message: f.message.clone(),
        }
    }
}

/// Results from running rules.
#[derive(Serialize, JsonSchema)]
pub struct RunRulesReport {
    pub findings: Vec<FindingItem>,
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub fixes_applied: usize,
}

impl OutputFormatter for RunRulesReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        for finding in &self.findings {
            out.push_str(&format!(
                "{}:{}:{}: [{}] {} ({})\n",
                finding.file,
                finding.line,
                finding.col,
                finding.severity,
                finding.message,
                finding.rule_id
            ));
        }
        if self.total == 0 {
            out.push_str("No findings.\n");
        } else {
            out.push_str(&format!(
                "\n{} finding(s): {} error(s), {} warning(s)\n",
                self.total, self.errors, self.warnings
            ));
        }
        if self.fixes_applied > 0 {
            out.push_str(&format!("Applied {} fix(es).\n", self.fixes_applied));
        }
        out
    }
}

/// A single rule entry for listing.
#[derive(Serialize, JsonSchema)]
pub struct RuleItem {
    pub id: String,
    pub severity: String,
    pub enabled: bool,
    pub builtin: bool,
    pub languages: Vec<String>,
    pub tags: Vec<String>,
    pub message: String,
}

impl From<&Rule> for RuleItem {
    fn from(r: &Rule) -> Self {
        Self {
            id: r.id.clone(),
            severity: r.severity.to_string(),
            enabled: r.enabled,
            builtin: r.builtin,
            languages: r.languages.clone(),
            tags: r.tags.clone(),
            message: r.message.clone(),
        }
    }
}

/// List of available rules.
#[derive(Serialize, JsonSchema)]
pub struct RulesListReport {
    pub rules: Vec<RuleItem>,
    pub total: usize,
}

impl OutputFormatter for RulesListReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        for rule in &self.rules {
            let status = if rule.enabled { "on" } else { "off" };
            let langs = if rule.languages.is_empty() {
                "all".to_string()
            } else {
                rule.languages.join(",")
            };
            out.push_str(&format!(
                "{:40} [{:7}] [{:3}] [{}]  {}\n",
                rule.id, rule.severity, status, langs, rule.message
            ));
        }
        out.push_str(&format!("\n{} rule(s) total\n", self.total));
        out
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
    root.map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {}", e))
}

// =============================================================================
// Service
// =============================================================================

/// Standalone CLI service for normalize-syntax-rules.
pub struct SyntaxRulesService;

impl SyntaxRulesService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SyntaxRulesService {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxRulesService {
    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

#[cli(
    name = "normalize-syntax-rules",
    version = "0.1.0",
    description = "Syntax-based linting rules with tree-sitter queries"
)]
impl SyntaxRulesService {
    /// Run rules against files in a directory
    #[cli(display_with = "display_output")]
    pub fn run(
        &self,
        #[param(
            positional,
            help = "Target directory or file (defaults to current directory)"
        )]
        target: Option<String>,
        #[param(short = 'r', help = "Project root (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Only run this specific rule ID")] rule: Option<String>,
        #[param(short = 't', help = "Only run rules with this tag")] tag: Option<String>,
        #[param(help = "Apply auto-fixes where available")] fix: bool,
        #[param(help = "Debug flags (comma-separated: timing, all)")] debug: Vec<String>,
    ) -> Result<RunRulesReport, String> {
        let project_root = resolve_root(root)?;
        let target_root = target
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| project_root.clone());

        let config = crate::RulesConfig::default();
        let rules = load_all_rules(&project_root, &config);

        let loader_arc = grammar_loader();
        let loader = &*loader_arc;
        let debug_flags = DebugFlags::from_args(&debug);

        let findings = run_rules(
            &rules,
            &target_root,
            &project_root,
            loader,
            rule.as_deref(),
            tag.as_deref(),
            None,
            &debug_flags,
            None,
        );

        let fixes_applied = if fix {
            apply_fixes(&findings).unwrap_or(0)
        } else {
            0
        };

        let errors = findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        let warnings = findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .count();
        let total = findings.len();

        let items: Vec<FindingItem> = findings.iter().map(FindingItem::from).collect();

        Ok(RunRulesReport {
            findings: items,
            total,
            errors,
            warnings,
            fixes_applied,
        })
    }

    /// List available rules
    #[cli(display_with = "display_output")]
    pub fn list(
        &self,
        #[param(short = 'r', help = "Project root (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(short = 't', help = "Filter by tag")] tag: Option<String>,
        #[param(help = "Show only enabled rules")] enabled: bool,
        #[param(help = "Show only disabled rules")] disabled: bool,
    ) -> Result<RulesListReport, String> {
        let project_root = resolve_root(root)?;
        let config = crate::RulesConfig::default();
        let rules = load_all_rules(&project_root, &config);

        let filtered: Vec<RuleItem> = rules
            .iter()
            .filter(|r| {
                if enabled && !r.enabled {
                    return false;
                }
                if disabled && r.enabled {
                    return false;
                }
                if let Some(ref t) = tag
                    && !r.tags.iter().any(|rt| rt == t)
                {
                    return false;
                }
                true
            })
            .map(RuleItem::from)
            .collect();

        let total = filtered.len();
        Ok(RulesListReport {
            rules: filtered,
            total,
        })
    }
}
