//! Parser for `elm.json` files (Elm package manager).
//!
//! Handles both application and package forms:
//! - Application: `dependencies.direct`, `test-dependencies.direct/indirect`
//! - Package: `dependencies`, `test-dependencies` (flat maps)

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use serde_json::Value;

/// Parser for `elm.json` files.
///
/// `dependencies.direct` (or `dependencies` in package form) → `Normal`.
/// `test-dependencies.*` → `Dev`.
/// `dependencies.indirect` is skipped (transitive, not declared by this project).
pub struct ElmParser;

impl ManifestParser for ElmParser {
    fn filename(&self) -> &'static str {
        "elm.json"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let json: Value =
            serde_json::from_str(content).map_err(|e| ManifestError(e.to_string()))?;

        let elm_type = json.get("type").and_then(|v| v.as_str());
        let is_application = elm_type == Some("application");

        // Package name from "name" field (only in package form)
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut deps = Vec::new();

        let dep_value = json.get("dependencies");
        if is_application {
            // Application form: { direct: {...}, indirect: {...} }
            if let Some(direct) = dep_value
                .and_then(|v| v.get("direct"))
                .and_then(|v| v.as_object())
            {
                for (pkg, ver) in direct {
                    deps.push(DeclaredDep {
                        name: pkg.clone(),
                        version_req: ver.as_str().map(|s| s.to_string()),
                        kind: DepKind::Normal,
                    });
                }
            }
        } else {
            // Package form: flat map
            if let Some(obj) = dep_value.and_then(|v| v.as_object()) {
                for (pkg, ver) in obj {
                    deps.push(DeclaredDep {
                        name: pkg.clone(),
                        version_req: ver.as_str().map(|s| s.to_string()),
                        kind: DepKind::Normal,
                    });
                }
            }
        }

        // test-dependencies: both direct and indirect treated as Dev
        if let Some(test_deps) = json.get("test-dependencies") {
            // Application form has direct/indirect sub-keys; package form is flat
            let direct_obj = test_deps.get("direct").and_then(|v| v.as_object());
            let indirect_obj = test_deps.get("indirect").and_then(|v| v.as_object());

            if direct_obj.is_some() || indirect_obj.is_some() {
                for obj in [direct_obj, indirect_obj].into_iter().flatten() {
                    for (pkg, ver) in obj {
                        deps.push(DeclaredDep {
                            name: pkg.clone(),
                            version_req: ver.as_str().map(|s| s.to_string()),
                            kind: DepKind::Dev,
                        });
                    }
                }
            } else if let Some(obj) = test_deps.as_object() {
                for (pkg, ver) in obj {
                    deps.push(DeclaredDep {
                        name: pkg.clone(),
                        version_req: ver.as_str().map(|s| s.to_string()),
                        kind: DepKind::Dev,
                    });
                }
            }
        }

        Ok(ParsedManifest {
            ecosystem: "elm",
            name,
            version,
            dependencies: deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_elm_application() {
        let content = r#"{
    "type": "application",
    "source-directories": ["src"],
    "elm-version": "0.19.1",
    "dependencies": {
        "direct": {
            "elm/core": "1.0.5",
            "elm/html": "1.0.0"
        },
        "indirect": {
            "elm/json": "1.1.3"
        }
    },
    "test-dependencies": {
        "direct": {
            "elm-explorations/test": "2.1.2"
        },
        "indirect": {}
    }
}"#;
        let m = ElmParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "elm");

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 2);
        assert!(normal.iter().any(|d| d.name == "elm/core"));
        assert!(normal.iter().any(|d| d.name == "elm/html"));

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "elm-explorations/test");
        assert_eq!(dev[0].version_req.as_deref(), Some("2.1.2"));
    }

    #[test]
    fn test_elm_package() {
        let content = r#"{
    "type": "package",
    "name": "elm/html",
    "summary": "Fast HTML",
    "version": "1.0.0",
    "elm-version": "0.19.0 <= v < 0.20.0",
    "exposed-modules": ["Html"],
    "dependencies": {
        "elm/core": "1.0.0 <= v < 2.0.0",
        "elm/virtual-dom": "1.0.0 <= v < 2.0.0"
    },
    "test-dependencies": {
        "elm-explorations/test": "1.0.0 <= v < 2.0.0"
    }
}"#;
        let m = ElmParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "elm");
        assert_eq!(m.name.as_deref(), Some("elm/html"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 2);

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "elm-explorations/test");
    }
}
