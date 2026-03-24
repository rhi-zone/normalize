//! Interactive rule setup wizard.
//!
//! Runs all rules against the codebase, groups violations by tag/category, and walks the
//! user through each group — offering per-rule enable/disable decisions as well as batch
//! operations for the whole group at once.

use std::collections::HashMap;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

use crate::runner::{RuleKind, RulesRunConfig, enable_disable, run_rules_report};
use crate::service::load_rules_config;

struct RuleMeta {
    message: String,
    severity: String,
    enabled: bool,
    rule_type: &'static str,
    recommended: bool,
    tags: Vec<String>,
}

fn paint_severity(severity: &str, use_colors: bool) -> String {
    if !use_colors {
        return severity.to_string();
    }
    match severity {
        "error" => nu_ansi_term::Color::Red.paint(severity).to_string(),
        "warning" => nu_ansi_term::Color::Yellow.paint(severity).to_string(),
        "info" => nu_ansi_term::Color::Cyan.paint(severity).to_string(),
        "hint" => nu_ansi_term::Color::DarkGray.paint(severity).to_string(),
        _ => severity.to_string(),
    }
}

/// Return a qualitative label for a violation count.
fn impact_label(count: usize) -> &'static str {
    match count {
        0 => "0 violations",
        1..=5 => "quick fix (1-5)",
        6..=50 => "moderate (6-50)",
        _ => "major cleanup (51+)",
    }
}

/// Primary tag for a rule — used for grouping. Falls back to "other".
fn primary_tag(tags: &[String]) -> &str {
    tags.first().map(String::as_str).unwrap_or("other")
}

/// Run the interactive setup wizard. Returns an exit code (0 = success).
pub fn run_setup_wizard(root: &Path) -> i32 {
    use normalize_facts_rules_interpret as interpret;

    let use_colors = io::stdout().is_terminal();

    println!("Rule Setup Wizard");
    println!("=================");
    println!("Running all rules against the codebase...\n");

    let config = load_rules_config(root);

    // Load rule metadata for descriptions
    let syntax_rules = normalize_syntax_rules::load_all_rules(root, &config.rules);
    let fact_rules = interpret::load_all_rules(root, &config.rules);

    // Build map: rule_id -> RuleMeta
    let mut rule_meta: HashMap<String, RuleMeta> = HashMap::new();
    for r in &syntax_rules {
        rule_meta.insert(
            r.id.clone(),
            RuleMeta {
                message: r.message.clone(),
                severity: r.severity.to_string(),
                enabled: r.enabled,
                rule_type: "syntax",
                recommended: r.recommended,
                tags: r.tags.clone(),
            },
        );
    }
    for r in &fact_rules {
        rule_meta.insert(
            r.id.clone(),
            RuleMeta {
                message: r.message.clone(),
                severity: r.severity.to_string(),
                enabled: r.enabled,
                rule_type: "fact",
                recommended: r.recommended,
                tags: r.tags.clone(),
            },
        );
    }

    // Run all rules to collect violations
    let rules_config = RulesRunConfig {
        rule_tags: config.rule_tags.clone(),
        rules: config.rules.clone(),
    };
    let report = run_rules_report(root, root, None, None, &RuleKind::All, &[], &rules_config);

    // Group issues by rule_id
    let mut by_rule: HashMap<String, Vec<normalize_output::diagnostics::Issue>> = HashMap::new();
    for issue in &report.issues {
        by_rule
            .entry(issue.rule_id.clone())
            .or_default()
            .push(issue.clone());
    }

    // Collect all rules that have violations
    let mut rules_with_violations: Vec<(String, Vec<normalize_output::diagnostics::Issue>)> =
        by_rule.into_iter().collect();

    if rules_with_violations.is_empty() {
        println!("No violations found — all rules pass on this codebase.");
        println!("Use `normalize rules list` to see available rules.");
        return 0;
    }

    // Group by primary tag
    let mut by_tag: HashMap<String, Vec<(String, Vec<normalize_output::diagnostics::Issue>)>> =
        HashMap::new();
    for (rule_id, issues) in rules_with_violations.drain(..) {
        let tag = rule_meta
            .get(&rule_id)
            .map(|m| primary_tag(&m.tags).to_string())
            .unwrap_or_else(|| "other".to_string());
        by_tag.entry(tag).or_default().push((rule_id, issues));
    }

    // Sort rules within each group: recommended first, then by violation count desc, then alpha
    for group in by_tag.values_mut() {
        group.sort_by(|a, b| {
            let a_rec = rule_meta.get(&a.0).is_some_and(|m| m.recommended);
            let b_rec = rule_meta.get(&b.0).is_some_and(|m| m.recommended);
            b_rec
                .cmp(&a_rec)
                .then(b.1.len().cmp(&a.1.len()))
                .then(a.0.cmp(&b.0))
        });
    }

    // Order groups: correctness first, then security, error-handling, bug-prone, style,
    // cleanup, architecture, complexity, documentation, then everything else alphabetically.
    let tag_order: &[&str] = &[
        "correctness",
        "security",
        "error-handling",
        "bug-prone",
        "style",
        "cleanup",
        "architecture",
        "complexity",
        "documentation",
        "readability",
        "performance",
    ];
    let mut sorted_tags: Vec<String> = by_tag.keys().cloned().collect();
    sorted_tags.sort_by(|a, b| {
        let ai = tag_order
            .iter()
            .position(|t| *t == a)
            .unwrap_or(tag_order.len());
        let bi = tag_order
            .iter()
            .position(|t| *t == b)
            .unwrap_or(tag_order.len());
        ai.cmp(&bi).then(a.cmp(b))
    });

    // Count rules with zero violations for summary
    let rules_with_zero =
        rule_meta.len() - sorted_tags.iter().map(|t| by_tag[t].len()).sum::<usize>();

    let total_rules: usize = sorted_tags.iter().map(|t| by_tag[t].len()).sum();
    println!(
        "Found violations from {} rules across {} files checked.",
        total_rules, report.files_checked
    );
    if rules_with_zero > 0 {
        println!(
            "{} rules had zero violations (use `normalize rules list` to see all).",
            rules_with_zero
        );
    }
    println!();
    println!("For each rule, choose:");
    println!("  [e]nable   — enable this rule (violations become errors/warnings)");
    println!("  [d]isable  — disable this rule (suppress violations)");
    println!("  [s]kip     — keep current setting (default, press Enter)");
    println!("  [q]uit     — stop here and keep remaining rules unchanged");
    println!();
    println!("Batch operations (shown after each group):");
    println!("  [ea]  — enable all rules in this group");
    println!("  [da]  — disable all rules in this group\n");

    let gray = nu_ansi_term::Color::DarkGray;
    let bold = nu_ansi_term::Style::new().bold();

    let stdin = io::stdin();
    let mut enabled_rules: Vec<String> = Vec::new();
    let mut disabled_rules: Vec<String> = Vec::new();
    let mut quit_early = false;

    let mut global_rule_num = 0usize;

    'outer: for tag in &sorted_tags {
        let group = &by_tag[tag];
        let group_len = group.len();

        // Group header
        let tag_header = format!("\n══ {} ══", tag.to_uppercase());
        if use_colors {
            println!("{}", nu_ansi_term::Color::Blue.bold().paint(&tag_header));
        } else {
            println!("{}", tag_header);
        }
        println!("  {} rule(s) with violations in this group\n", group_len);

        // Collect IDs for batch operations (processed at group end)
        let group_ids: Vec<String> = group.iter().map(|(id, _)| id.clone()).collect();

        for (rule_id, issues) in group {
            global_rule_num += 1;

            let meta = rule_meta.get(rule_id);
            let enabled = meta.is_some_and(|m| m.enabled);
            let severity = meta.map_or("info", |m| m.severity.as_str());
            let rule_type = meta.map_or("?", |m| m.rule_type);
            let description = meta.map_or("", |m| m.message.as_str());
            let recommended = meta.is_some_and(|m| m.recommended);

            // Rule header
            let sev_colored = paint_severity(severity, use_colors);
            let state = if enabled { "enabled" } else { "disabled" };
            let state_str = if use_colors {
                if enabled {
                    nu_ansi_term::Color::Green.paint(state).to_string()
                } else {
                    gray.paint(state).to_string()
                }
            } else {
                state.to_string()
            };

            let rec_marker = if recommended {
                if use_colors {
                    format!(" {}", nu_ansi_term::Color::Yellow.paint("recommended"))
                } else {
                    " recommended".to_string()
                }
            } else {
                String::new()
            };

            let impact = impact_label(issues.len());
            let impact_str = if use_colors {
                gray.paint(impact).to_string()
            } else {
                impact.to_string()
            };

            println!(
                "─── ({}/{}) {} [{}] {}{} ───",
                global_rule_num,
                total_rules,
                if use_colors {
                    bold.paint(rule_id.as_str()).to_string()
                } else {
                    rule_id.clone()
                },
                rule_type,
                state_str,
                rec_marker
            );
            println!(
                "    Severity: {}  |  {} violations  [{}]",
                sev_colored,
                issues.len(),
                impact_str
            );
            if !description.is_empty() {
                println!("    {}", description);
            }
            println!();

            // Show up to 5 example violations
            let sample = issues.iter().take(5);
            for issue in sample {
                let location = match issue.line {
                    Some(line) => format!("{}:{}", issue.file, line),
                    None => issue.file.clone(),
                };
                let loc_str = if use_colors {
                    gray.paint(&location).to_string()
                } else {
                    location
                };
                println!("    {} — {}", loc_str, issue.message);
            }
            if issues.len() > 5 {
                let more = issues.len() - 5;
                let more_str = format!("    ... and {} more", more);
                if use_colors {
                    println!("{}", gray.paint(&more_str));
                } else {
                    println!("{}", more_str);
                }
            }
            println!();

            // Prompt
            let prompt = format!("    [e]nable / [d]isable / [s]kip (current: {}) > ", state);
            print!("{}", prompt);
            io::stdout().flush().ok();

            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() {
                eprintln!("Failed to read input");
                return 1;
            }

            match line.trim().to_lowercase().as_str() {
                "e" | "enable" => {
                    if !enabled {
                        match enable_disable(root, rule_id, true, false, &rules_config) {
                            Ok(_) => {
                                println!("  → Enabled {}", rule_id);
                                enabled_rules.push(rule_id.clone());
                            }
                            Err(e) => eprintln!("  Error enabling {}: {}", rule_id, e),
                        }
                    } else {
                        println!("  → Already enabled");
                    }
                }
                "d" | "disable" => {
                    if enabled {
                        match enable_disable(root, rule_id, false, false, &rules_config) {
                            Ok(_) => {
                                println!("  → Disabled {}", rule_id);
                                disabled_rules.push(rule_id.clone());
                            }
                            Err(e) => eprintln!("  Error disabling {}: {}", rule_id, e),
                        }
                    } else {
                        println!("  → Already disabled");
                    }
                }
                "q" | "quit" => {
                    println!("\nStopped at rule {}/{}", global_rule_num, total_rules);
                    quit_early = true;
                    break 'outer;
                }
                _ => {
                    // skip or empty
                    println!("  → Skipped");
                }
            }
            println!();
        }

        // Batch operations for the group
        if !quit_early {
            let batch_prompt = format!(
                "  [ea] enable all {} / [da] disable all {} / [s]kip group > ",
                tag, tag
            );
            print!("{}", batch_prompt);
            io::stdout().flush().ok();

            let mut batch_line = String::new();
            if stdin.lock().read_line(&mut batch_line).is_err() {
                eprintln!("Failed to read input");
                return 1;
            }

            match batch_line.trim().to_lowercase().as_str() {
                "ea" => {
                    println!("  → Enabling all {} rules in [{}]:", group_ids.len(), tag);
                    for id in &group_ids {
                        let currently_enabled = rule_meta.get(id).is_some_and(|m| m.enabled)
                            || enabled_rules.contains(id);
                        let already_disabled = disabled_rules.contains(id);
                        // If we just disabled it in this session, re-enable
                        if !currently_enabled || already_disabled {
                            match enable_disable(root, id, true, false, &rules_config) {
                                Ok(_) => {
                                    println!("      enabled {}", id);
                                    enabled_rules.push(id.clone());
                                    // Remove from disabled if it was just disabled
                                    disabled_rules.retain(|x| x != id);
                                }
                                Err(e) => eprintln!("      Error enabling {}: {}", id, e),
                            }
                        } else {
                            println!("      {} (already enabled)", id);
                        }
                    }
                }
                "da" => {
                    println!("  → Disabling all {} rules in [{}]:", group_ids.len(), tag);
                    for id in &group_ids {
                        let currently_enabled = rule_meta.get(id).is_some_and(|m| m.enabled)
                            || enabled_rules.contains(id);
                        if currently_enabled {
                            match enable_disable(root, id, false, false, &rules_config) {
                                Ok(_) => {
                                    println!("      disabled {}", id);
                                    disabled_rules.push(id.clone());
                                    // Remove from enabled if it was just enabled
                                    enabled_rules.retain(|x| x != id);
                                }
                                Err(e) => eprintln!("      Error disabling {}: {}", id, e),
                            }
                        } else {
                            println!("      {} (already disabled)", id);
                        }
                    }
                }
                "q" | "quit" => {
                    println!("\nStopped after group [{}].", tag);
                    quit_early = true;
                    break 'outer;
                }
                _ => {
                    println!("  → Skipped group");
                }
            }
            println!();
        }
    }

    if quit_early && enabled_rules.is_empty() && disabled_rules.is_empty() {
        println!("Setup cancelled. No changes were made.");
    } else if enabled_rules.is_empty() && disabled_rules.is_empty() {
        println!("Setup complete. No changes made.");
        println!("Run 'normalize rules show-config' to see the current configuration.");
    } else {
        println!("Setup complete. Changes made:\n");
        if !enabled_rules.is_empty() {
            println!("  Enabled ({}):", enabled_rules.len());
            for id in &enabled_rules {
                println!("    {}", id);
            }
        }
        if !disabled_rules.is_empty() {
            println!("  Disabled ({}):", disabled_rules.len());
            for id in &disabled_rules {
                println!("    {}", id);
            }
        }
        println!("\nConfiguration saved to .normalize/config.toml");
        println!("Run 'normalize rules show-config' to see the full configuration.");
    }

    0
}
