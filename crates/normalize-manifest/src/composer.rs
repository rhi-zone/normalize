//! Parser for `composer.json` files (PHP/Composer).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use serde_json::Value;

/// Parser for `composer.json` files.
pub struct ComposerParser;

impl ManifestParser for ComposerParser {
    fn filename(&self) -> &'static str {
        "composer.json"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let json: Value =
            serde_json::from_str(content).map_err(|e| ManifestError(e.to_string()))?;

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut deps = Vec::new();
        extract_composer_deps(&json, "require", DepKind::Normal, &mut deps);
        extract_composer_deps(&json, "require-dev", DepKind::Dev, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "composer",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_composer_deps(json: &Value, field: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    let Some(obj) = json.get(field).and_then(|v| v.as_object()) else {
        return;
    };
    for (name, ver) in obj {
        // Skip platform requirements (php, ext-*, lib-*)
        if name == "php" || name.starts_with("ext-") || name.starts_with("lib-") {
            continue;
        }
        out.push(DeclaredDep {
            name: name.clone(),
            version_req: ver.as_str().map(|s| s.to_string()),
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_composer_json() {
        let content = r#"{
  "name": "vendor/my-package",
  "version": "1.0.0",
  "require": {
    "php": ">=8.1",
    "ext-json": "*",
    "symfony/framework-bundle": "^6.0",
    "doctrine/orm": "^2.13"
  },
  "require-dev": {
    "phpunit/phpunit": "^10.0"
  }
}"#;
        let m = ComposerParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "composer");
        assert_eq!(m.name.as_deref(), Some("vendor/my-package"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));

        // php and ext-json are filtered out
        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 2);
        assert!(normal.iter().any(|d| d.name == "symfony/framework-bundle"));
        assert!(normal.iter().any(|d| d.name == "doctrine/orm"));

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "phpunit/phpunit");
    }
}
