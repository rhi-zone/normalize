//! Parser for `gleam.toml` files (Gleam package manager).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use toml::Value;

/// Parser for `gleam.toml` files.
///
/// `[dependencies]` → `Normal`, `[dev-dependencies]` → `Dev`.
/// Version values are used as-is (e.g., `">= 0.34.0 and < 2.0.0"`).
pub struct GleamParser;

impl ManifestParser for GleamParser {
    fn filename(&self) -> &'static str {
        "gleam.toml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let toml: Value = content
            .parse::<Value>()
            .map_err(|e| ManifestError(e.to_string()))?;

        let name = toml
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let version = toml
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut deps = Vec::new();
        extract_gleam_deps(&toml, "dependencies", DepKind::Normal, &mut deps);
        extract_gleam_deps(&toml, "dev-dependencies", DepKind::Dev, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "gleam",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_gleam_deps(toml: &Value, section: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    let Some(table) = toml.get(section).and_then(|v| v.as_table()) else {
        return;
    };
    for (name, val) in table {
        let version_req = val.as_str().map(|s| s.to_string());
        out.push(DeclaredDep {
            name: name.clone(),
            version_req,
            kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_gleam_toml() {
        let content = r#"
name = "my_project"
version = "0.1.0"

[dependencies]
gleam_stdlib = ">= 0.34.0 and < 2.0.0"
lustre = ">= 4.0.0 and < 5.0.0"

[dev-dependencies]
gleeunit = ">= 1.0.0 and < 2.0.0"
"#;
        let m = GleamParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "gleam");
        assert_eq!(m.name.as_deref(), Some("my_project"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));
        assert_eq!(m.dependencies.len(), 3);

        let stdlib = m
            .dependencies
            .iter()
            .find(|d| d.name == "gleam_stdlib")
            .unwrap();
        assert_eq!(stdlib.kind, DepKind::Normal);
        assert_eq!(stdlib.version_req.as_deref(), Some(">= 0.34.0 and < 2.0.0"));

        let lustre = m.dependencies.iter().find(|d| d.name == "lustre").unwrap();
        assert_eq!(lustre.kind, DepKind::Normal);

        let gleeunit = m
            .dependencies
            .iter()
            .find(|d| d.name == "gleeunit")
            .unwrap();
        assert_eq!(gleeunit.kind, DepKind::Dev);
        assert_eq!(
            gleeunit.version_req.as_deref(),
            Some(">= 1.0.0 and < 2.0.0")
        );
    }

    #[test]
    fn test_gleam_no_dev_deps() {
        let content = r#"
name = "lib"
version = "0.2.0"

[dependencies]
gleam_stdlib = ">= 0.30.0 and < 2.0.0"
"#;
        let m = GleamParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].kind, DepKind::Normal);
    }
}
