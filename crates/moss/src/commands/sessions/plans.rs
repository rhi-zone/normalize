//! Plans command - list and view Claude Code plans from ~/.claude/plans/

use crate::output::OutputFormatter;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// Plan item for serialization
#[derive(Debug, Serialize)]
struct PlanListItem {
    name: String,
    title: String,
    modified: String,
    size: u64,
}

/// Plans list report
#[derive(Debug, Serialize)]
struct PlansListReport {
    plans: Vec<PlanListItem>,
}

impl OutputFormatter for PlansListReport {
    fn format_text(&self) -> String {
        let mut lines = Vec::new();
        for plan in &self.plans {
            lines.push(format!(
                "{} [{}] {} ({}B)",
                plan.modified, plan.name, plan.title, plan.size
            ));
        }
        lines.push(String::new());
        lines.push(format!("{} plans found", self.plans.len()));
        lines.join("\n")
    }
}

/// Get the plans directory path
fn plans_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("plans"))
}

/// Plan metadata extracted from a plan file
struct PlanInfo {
    name: String,
    title: String,
    modified: std::time::SystemTime,
    size: u64,
}

/// Extract title from plan file (first line: "# Plan: <title>")
fn extract_title(content: &str) -> String {
    // Not chained: second strip_prefix is a fallback, would require duplicating first check
    if let Some(first_line) = content.lines().next() {
        if let Some(title) = first_line.strip_prefix("# Plan: ") {
            return title.to_string();
        }
        if let Some(title) = first_line.strip_prefix("# ") {
            return title.to_string();
        }
    }
    "(untitled)".to_string()
}

/// List all plans
fn list_plans(limit: usize) -> Vec<PlanInfo> {
    let Some(dir) = plans_dir() else {
        return vec![];
    };

    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![];
    };

    let mut plans: Vec<PlanInfo> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
        .filter_map(|e| {
            let path = e.path();
            let name = path.file_stem()?.to_string_lossy().to_string();
            let meta = e.metadata().ok()?;
            let content = fs::read_to_string(&path).ok()?;
            let title = extract_title(&content);
            Some(PlanInfo {
                name,
                title,
                modified: meta.modified().ok()?,
                size: meta.len(),
            })
        })
        .collect();

    // Sort by modification time (newest first)
    plans.sort_by(|a, b| b.modified.cmp(&a.modified));
    plans.truncate(limit);
    plans
}

/// Format a SystemTime as a human-readable date
fn format_time(time: std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Simple date formatting (YYYY-MM-DD HH:MM)
    let days_since_epoch = secs / 86400;
    let remaining_secs = secs % 86400;
    let hours = remaining_secs / 3600;
    let minutes = (remaining_secs % 3600) / 60;

    // Calculate date (simplified, doesn't account for leap years perfectly)
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;

    while remaining_days >= 365 {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days >= days_in_year {
            remaining_days -= days_in_year;
            year += 1;
        } else {
            break;
        }
    }

    let days_in_months: [u64; 12] = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days in days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }

    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        year, month, day, hours, minutes
    )
}

/// Main command handler
pub fn cmd_plans(name: Option<&str>, limit: usize, json: bool) -> i32 {
    let Some(dir) = plans_dir() else {
        eprintln!("Could not find home directory");
        return 1;
    };

    if !dir.exists() {
        if json {
            println!("[]");
        } else {
            eprintln!("No plans directory found at {}", dir.display());
        }
        return 0;
    }

    if let Some(plan_name) = name {
        // View specific plan
        let plan_path = dir.join(format!("{}.md", plan_name));
        if !plan_path.exists() {
            // Try fuzzy match
            let plans = list_plans(100);
            let matches: Vec<_> = plans
                .iter()
                .filter(|p| {
                    p.name.contains(plan_name)
                        || p.title.to_lowercase().contains(&plan_name.to_lowercase())
                })
                .collect();

            if matches.is_empty() {
                eprintln!("Plan not found: {}", plan_name);
                return 1;
            } else if matches.len() == 1 {
                // Single match, show it
                let plan_path = dir.join(format!("{}.md", matches[0].name));
                match fs::read_to_string(&plan_path) {
                    Ok(content) => {
                        if json {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "name": matches[0].name,
                                    "title": matches[0].title,
                                    "content": content
                                })
                            );
                        } else {
                            print!("{}", content);
                        }
                        return 0;
                    }
                    Err(e) => {
                        eprintln!("Error reading plan: {}", e);
                        return 1;
                    }
                }
            } else {
                eprintln!("Multiple matches for '{}' - be more specific:", plan_name);
                for m in matches {
                    eprintln!("  {} - {}", m.name, m.title);
                }
                return 1;
            }
        }

        match fs::read_to_string(&plan_path) {
            Ok(content) => {
                if json {
                    let title = extract_title(&content);
                    println!(
                        "{}",
                        serde_json::json!({
                            "name": plan_name,
                            "title": title,
                            "content": content
                        })
                    );
                } else {
                    print!("{}", content);
                }
                0
            }
            Err(e) => {
                eprintln!("Error reading plan: {}", e);
                1
            }
        }
    } else {
        // List all plans
        let plans = list_plans(limit);

        if plans.is_empty() {
            eprintln!("No plans found in {}", dir.display());
            return 0;
        }

        let items: Vec<PlanListItem> = plans
            .iter()
            .map(|p| PlanListItem {
                name: p.name.clone(),
                title: p.title.clone(),
                modified: format_time(p.modified),
                size: p.size,
            })
            .collect();

        let report = PlansListReport { plans: items };
        let config = crate::config::MossConfig::default();
        let format =
            crate::output::OutputFormat::from_cli(json, None, false, false, &config.pretty);
        report.print(&format);
        0
    }
}
