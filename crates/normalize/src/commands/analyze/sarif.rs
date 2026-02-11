//! SARIF output for analysis passes.
//!
//! Generates SARIF 2.1.0 formatted output for IDE integration.

use crate::analyze::complexity::{FunctionComplexity, RiskLevel};
use crate::analyze::function_length::{FunctionLength, LengthCategory};
use std::path::Path;

/// SARIF severity level.
fn risk_to_sarif_level(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low | RiskLevel::Moderate => "note",
        RiskLevel::High => "warning",
        RiskLevel::Critical => "error",
    }
}

/// SARIF severity level for function length.
fn length_to_sarif_level(category: LengthCategory) -> &'static str {
    match category {
        LengthCategory::Short | LengthCategory::Medium => "note",
        LengthCategory::Long => "warning",
        LengthCategory::TooLong => "error",
    }
}

/// Print complexity report in SARIF format.
pub fn print_complexity_sarif(functions: &[FunctionComplexity], root: &Path) {
    let rules = vec![
        serde_json::json!({
            "id": "high-complexity",
            "shortDescription": { "text": "Function has high cyclomatic complexity (11-20)" },
            "defaultConfiguration": { "level": "warning" },
            "properties": { "threshold": 11 }
        }),
        serde_json::json!({
            "id": "critical-complexity",
            "shortDescription": { "text": "Function has critical cyclomatic complexity (21+)" },
            "defaultConfiguration": { "level": "error" },
            "properties": { "threshold": 21 }
        }),
    ];

    let results: Vec<_> = functions
        .iter()
        .filter(|f| f.risk_level() == RiskLevel::High || f.risk_level() == RiskLevel::Critical)
        .map(|f| {
            let rule_id = match f.risk_level() {
                RiskLevel::Critical => "critical-complexity",
                _ => "high-complexity",
            };

            let uri = f
                .file_path
                .as_ref()
                .map(|p| {
                    let path = Path::new(p);
                    path.canonicalize()
                        .ok()
                        .map(|abs| format!("file://{}", abs.display()))
                        .unwrap_or_else(|| p.clone())
                })
                .unwrap_or_default();

            serde_json::json!({
                "ruleId": rule_id,
                "level": risk_to_sarif_level(f.risk_level()),
                "message": {
                    "text": format!(
                        "Function '{}' has cyclomatic complexity of {} ({})",
                        f.short_name(),
                        f.complexity,
                        f.risk_level().as_str()
                    )
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri },
                        "region": {
                            "startLine": f.start_line,
                            "endLine": f.end_line
                        }
                    }
                }]
            })
        })
        .collect();

    let sarif = serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "normalize",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/rhi-zone/normalize",
                    "rules": rules
                }
            },
            "results": results,
            "invocations": [{
                "executionSuccessful": true,
                "workingDirectory": {
                    "uri": format!("file://{}", root.canonicalize().unwrap_or_else(|_| root.to_path_buf()).display())
                }
            }]
        }]
    });

    println!("{}", serde_json::to_string_pretty(&sarif).unwrap());
}

/// Print function length report in SARIF format.
pub fn print_length_sarif(functions: &[FunctionLength], root: &Path) {
    let rules = vec![
        serde_json::json!({
            "id": "long-function",
            "shortDescription": { "text": "Function is getting long (51-100 lines)" },
            "defaultConfiguration": { "level": "warning" },
            "properties": { "threshold": 51 }
        }),
        serde_json::json!({
            "id": "too-long-function",
            "shortDescription": { "text": "Function is too long (100+ lines)" },
            "defaultConfiguration": { "level": "error" },
            "properties": { "threshold": 100 }
        }),
    ];

    let results: Vec<_> = functions
        .iter()
        .filter(|f| f.category() == LengthCategory::Long || f.category() == LengthCategory::TooLong)
        .map(|f| {
            let rule_id = match f.category() {
                LengthCategory::TooLong => "too-long-function",
                _ => "long-function",
            };

            let uri = f
                .file_path
                .as_ref()
                .map(|p| {
                    let path = Path::new(p);
                    path.canonicalize()
                        .ok()
                        .map(|abs| format!("file://{}", abs.display()))
                        .unwrap_or_else(|| p.clone())
                })
                .unwrap_or_default();

            serde_json::json!({
                "ruleId": rule_id,
                "level": length_to_sarif_level(f.category()),
                "message": {
                    "text": format!(
                        "Function '{}' is {} lines ({})",
                        f.short_name(),
                        f.lines,
                        f.category().as_str()
                    )
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": uri },
                        "region": {
                            "startLine": f.start_line,
                            "endLine": f.end_line
                        }
                    }
                }]
            })
        })
        .collect();

    let sarif = serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "normalize",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/rhi-zone/normalize",
                    "rules": rules
                }
            },
            "results": results,
            "invocations": [{
                "executionSuccessful": true,
                "workingDirectory": {
                    "uri": format!("file://{}", root.canonicalize().unwrap_or_else(|_| root.to_path_buf()).display())
                }
            }]
        }]
    });

    println!("{}", serde_json::to_string_pretty(&sarif).unwrap());
}
