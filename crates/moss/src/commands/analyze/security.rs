//! Security analysis - run security scanning tools

use super::report::{SecurityFinding, SecurityReport, Severity};
use std::path::Path;
use std::process::Command;

/// Check if a command is available on the system
fn command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run bandit security scanner on Python code
fn run_bandit(root: &Path) -> Result<Vec<SecurityFinding>, String> {
    let output = Command::new("bandit")
        .args(["-r", "-f", "json", "-q"])
        .arg(root)
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    let json: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| e.to_string())?;

    let mut findings = Vec::new();
    if let Some(results) = json.get("results").and_then(|r| r.as_array()) {
        for result in results {
            let file = result
                .get("filename")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let line = result
                .get("line_number")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let severity_str = result
                .get("issue_severity")
                .and_then(|v| v.as_str())
                .unwrap_or("low");
            let rule_id = result
                .get("test_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let message = result
                .get("issue_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            findings.push(SecurityFinding {
                file,
                line,
                severity: Severity::from_str(severity_str),
                rule_id,
                message,
                tool: "bandit".to_string(),
            });
        }
    }

    Ok(findings)
}

/// Run security analysis on a codebase
pub fn analyze_security(root: &Path) -> SecurityReport {
    let mut report = SecurityReport::default();

    if command_available("bandit") {
        match run_bandit(root) {
            Ok(findings) => {
                report.findings.extend(findings);
                report.tools_run.push("bandit".to_string());
            }
            Err(_) => {
                report.tools_skipped.push("bandit (error)".to_string());
            }
        }
    } else {
        report.tools_skipped.push("bandit".to_string());
    }

    report
}
