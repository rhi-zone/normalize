//! Parser for `gradle/libs.versions.toml` files (Gradle version catalog).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use std::collections::HashMap;
use toml::Value;

/// Parser for `gradle/libs.versions.toml` (Gradle version catalog).
///
/// Parses `[libraries]` entries and resolves `version.ref` from `[versions]`.
/// Module format is `group:artifact` — used as the dependency name.
/// `[bundles]` and `[plugins]` are skipped.
pub struct GradleLibsParser;

impl ManifestParser for GradleLibsParser {
    fn filename(&self) -> &'static str {
        "libs.versions.toml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let toml: Value = content
            .parse::<Value>()
            .map_err(|e| ManifestError(e.to_string()))?;

        // Build versions map: alias -> version string
        let versions: HashMap<String, String> = toml
            .get("versions")
            .and_then(|v| v.as_table())
            .map(|t| {
                t.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let mut deps = Vec::new();

        if let Some(libraries) = toml.get("libraries").and_then(|v| v.as_table()) {
            for (_alias, entry) in libraries {
                let Some(table) = entry.as_table() else {
                    continue;
                };

                // module = "group:artifact"
                let Some(module) = table.get("module").and_then(|v| v.as_str()) else {
                    continue;
                };

                // Resolve version: version.ref -> versions table, or version = "..."
                let version_req = if let Some(vref) = table
                    .get("version")
                    .and_then(|v| v.as_table())
                    .and_then(|t| t.get("ref"))
                    .and_then(|v| v.as_str())
                {
                    versions.get(vref).cloned()
                } else {
                    table
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                };

                deps.push(DeclaredDep {
                    name: module.to_string(),
                    version_req,
                    kind: DepKind::Normal,
                });
            }
        }

        Ok(ParsedManifest {
            ecosystem: "gradle",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_gradle_libs_version_catalog() {
        let content = r#"
[versions]
junit = "4.13.2"
retrofit = "2.9.0"

[libraries]
junit = { module = "junit:junit", version.ref = "junit" }
retrofit = { module = "com.squareup.retrofit2:retrofit", version.ref = "retrofit" }
retrofit-gson = { module = "com.squareup.retrofit2:converter-gson", version = "2.9.0" }

[bundles]
retrofit = ["retrofit", "retrofit-gson"]
"#;
        let m = GradleLibsParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "gradle");
        assert_eq!(m.dependencies.len(), 3);

        let junit = m
            .dependencies
            .iter()
            .find(|d| d.name == "junit:junit")
            .unwrap();
        assert_eq!(junit.version_req.as_deref(), Some("4.13.2"));
        assert_eq!(junit.kind, DepKind::Normal);

        let retrofit = m
            .dependencies
            .iter()
            .find(|d| d.name == "com.squareup.retrofit2:retrofit")
            .unwrap();
        assert_eq!(retrofit.version_req.as_deref(), Some("2.9.0"));

        let gson = m
            .dependencies
            .iter()
            .find(|d| d.name == "com.squareup.retrofit2:converter-gson")
            .unwrap();
        assert_eq!(gson.version_req.as_deref(), Some("2.9.0"));
    }

    #[test]
    fn test_gradle_libs_no_versions_section() {
        let content = r#"
[libraries]
mylib = { module = "com.example:mylib", version = "1.0.0" }
"#;
        let m = GradleLibsParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "com.example:mylib");
        assert_eq!(m.dependencies[0].version_req.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn test_gradle_libs_unresolvable_ref_gives_none() {
        let content = r#"
[versions]
# intentionally empty

[libraries]
orphan = { module = "com.example:orphan", version.ref = "nonexistent" }
"#;
        let m = GradleLibsParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert!(m.dependencies[0].version_req.is_none());
    }
}
