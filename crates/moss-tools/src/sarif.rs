//! SARIF 2.1.0 format - both parsing and generation.
//!
//! SARIF (Static Analysis Results Interchange Format) is a standard format
//! for static analysis tool output. Supported by GitHub, VS Code, and many CI systems.
//!
//! This module supports:
//! - Generating SARIF from diagnostics (for output)
//! - Parsing SARIF from external tools (for consumption)

use crate::{Diagnostic, DiagnosticSeverity, Fix, Location};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SARIF 2.1.0 report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifReport {
    #[serde(rename = "$schema", default)]
    pub schema: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRun {
    pub tool: SarifTool,
    #[serde(default)]
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifDriver {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub information_uri: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRule {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub short_description: Option<SarifMessage>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub help_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    pub rule_id: String,
    #[serde(default = "default_level")]
    pub level: String,
    pub message: SarifMessage,
    #[serde(default)]
    pub locations: Vec<SarifLocation>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fixes: Vec<SarifFix>,
}

fn default_level() -> String {
    "warning".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifPhysicalLocation {
    pub artifact_location: SarifArtifactLocation,
    #[serde(default)]
    pub region: SarifRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifArtifactLocation {
    pub uri: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRegion {
    #[serde(default = "default_one")]
    pub start_line: usize,
    #[serde(default = "default_one")]
    pub start_column: usize,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub end_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub end_column: Option<usize>,
}

fn default_one() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifFix {
    pub description: SarifMessage,
    #[serde(default)]
    pub artifact_changes: Vec<SarifArtifactChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifArtifactChange {
    pub artifact_location: SarifArtifactLocation,
    #[serde(default)]
    pub replacements: Vec<SarifReplacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifReplacement {
    pub deleted_region: SarifRegion,
    pub inserted_content: SarifContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifContent {
    pub text: String,
}

impl SarifReport {
    /// Parse SARIF from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Convert SARIF report to diagnostics.
    ///
    /// Extracts all results from all runs and converts them to our Diagnostic format.
    pub fn to_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for run in &self.runs {
            let tool_name = &run.tool.driver.name;

            // Build a map of rule IDs to help URIs
            let rule_help: HashMap<&str, Option<&str>> = run
                .tool
                .driver
                .rules
                .iter()
                .map(|r| (r.id.as_str(), r.help_uri.as_deref()))
                .collect();

            for result in &run.results {
                let severity = DiagnosticSeverity::from_sarif_level(&result.level);

                // Get location from first location entry, or use defaults
                let (file, line, column, end_line, end_column) =
                    if let Some(loc) = result.locations.first() {
                        let phys = &loc.physical_location;
                        (
                            phys.artifact_location.uri.clone(),
                            phys.region.start_line,
                            phys.region.start_column,
                            phys.region.end_line,
                            phys.region.end_column,
                        )
                    } else {
                        ("unknown".to_string(), 1, 1, None, None)
                    };

                // Extract fix if present
                let fix = result.fixes.first().map(|f| {
                    let replacement = f
                        .artifact_changes
                        .first()
                        .and_then(|ac| ac.replacements.first())
                        .map(|r| r.inserted_content.text.clone())
                        .unwrap_or_default();
                    Fix {
                        description: f.description.text.clone(),
                        replacement,
                    }
                });

                diagnostics.push(Diagnostic {
                    tool: tool_name.clone(),
                    rule_id: result.rule_id.clone(),
                    message: result.message.text.clone(),
                    severity,
                    location: Location {
                        file: file.into(),
                        line,
                        column,
                        end_line,
                        end_column,
                    },
                    fix,
                    help_url: rule_help
                        .get(result.rule_id.as_str())
                        .copied()
                        .flatten()
                        .map(String::from),
                });
            }
        }

        diagnostics
    }

    /// Create a SARIF report from diagnostics.
    pub fn from_diagnostics(diagnostics: &[Diagnostic]) -> Self {
        // Group diagnostics by tool
        let mut by_tool: HashMap<&str, Vec<&Diagnostic>> = HashMap::new();
        for d in diagnostics {
            by_tool.entry(&d.tool).or_default().push(d);
        }

        let runs = by_tool
            .into_iter()
            .map(|(tool_name, diags)| {
                // Collect unique rules
                let mut rules_map: HashMap<&str, SarifRule> = HashMap::new();
                for d in &diags {
                    rules_map.entry(&d.rule_id).or_insert_with(|| SarifRule {
                        id: d.rule_id.clone(),
                        short_description: Some(SarifMessage {
                            text: d.message.clone(),
                        }),
                        help_uri: d.help_url.clone(),
                    });
                }

                let results = diags
                    .iter()
                    .map(|d| {
                        let fixes = if let Some(fix) = &d.fix {
                            vec![SarifFix {
                                description: SarifMessage {
                                    text: fix.description.clone(),
                                },
                                artifact_changes: vec![SarifArtifactChange {
                                    artifact_location: SarifArtifactLocation {
                                        uri: d.location.file.display().to_string(),
                                    },
                                    replacements: vec![SarifReplacement {
                                        deleted_region: SarifRegion {
                                            start_line: d.location.line,
                                            start_column: d.location.column,
                                            end_line: d.location.end_line,
                                            end_column: d.location.end_column,
                                        },
                                        inserted_content: SarifContent {
                                            text: fix.replacement.clone(),
                                        },
                                    }],
                                }],
                            }]
                        } else {
                            vec![]
                        };

                        SarifResult {
                            rule_id: d.rule_id.clone(),
                            level: d.severity.to_sarif_level().to_string(),
                            message: SarifMessage {
                                text: d.message.clone(),
                            },
                            locations: vec![SarifLocation {
                                physical_location: SarifPhysicalLocation {
                                    artifact_location: SarifArtifactLocation {
                                        uri: d.location.file.display().to_string(),
                                    },
                                    region: SarifRegion {
                                        start_line: d.location.line,
                                        start_column: d.location.column,
                                        end_line: d.location.end_line,
                                        end_column: d.location.end_column,
                                    },
                                },
                            }],
                            fixes,
                        }
                    })
                    .collect();

                SarifRun {
                    tool: SarifTool {
                        driver: SarifDriver {
                            name: tool_name.to_string(),
                            version: None,
                            information_uri: None,
                            rules: rules_map.into_values().collect(),
                        },
                    },
                    results,
                }
            })
            .collect();

        SarifReport {
            schema: "https://json.schemastore.org/sarif-2.1.0.json".to_string(),
            version: "2.1.0".to_string(),
            runs,
        }
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}
