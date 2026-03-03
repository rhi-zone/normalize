//! Parser for `Cargo.toml` files (Rust/Cargo).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use toml::Value;

/// Parser for `Cargo.toml` files.
pub struct CargoParser;

impl ManifestParser for CargoParser {
    fn filename(&self) -> &'static str {
        "Cargo.toml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let toml: Value = content
            .parse::<Value>()
            .map_err(|e| ManifestError(e.to_string()))?;

        let package = toml.get("package");
        let name = package
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let version = package
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut deps = Vec::new();
        extract_cargo_deps(&toml, "dependencies", DepKind::Normal, &mut deps);
        extract_cargo_deps(&toml, "dev-dependencies", DepKind::Dev, &mut deps);
        extract_cargo_deps(&toml, "build-dependencies", DepKind::Build, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "cargo",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_cargo_deps(toml: &Value, section: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    let Some(table) = toml.get(section).and_then(|v| v.as_table()) else {
        return;
    };
    for (name, val) in table {
        let version_req = if let Some(s) = val.as_str() {
            Some(s.to_string())
        } else if let Some(t) = val.as_table() {
            t.get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };
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
    fn test_parse_cargo_toml() {
        let content = r#"
[package]
name = "my-crate"
version = "0.2.0"
edition = "2024"

[dependencies]
serde = "1"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"

[build-dependencies]
cc = "1"
"#;
        let m = CargoParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "cargo");
        assert_eq!(m.name.as_deref(), Some("my-crate"));
        assert_eq!(m.version.as_deref(), Some("0.2.0"));

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        assert_eq!(normal.len(), 2);

        let serde_dep = normal.iter().find(|d| d.name == "serde").unwrap();
        assert_eq!(serde_dep.version_req.as_deref(), Some("1"));

        let tokio_dep = normal.iter().find(|d| d.name == "tokio").unwrap();
        assert_eq!(tokio_dep.version_req.as_deref(), Some("1"));

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "tempfile");

        let build: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Build)
            .collect();
        assert_eq!(build.len(), 1);
        assert_eq!(build[0].name, "cc");
    }

    #[test]
    fn test_no_package_section() {
        // Workspace-only Cargo.toml
        let content = "[workspace]\nmembers = [\"crates/*\"]\n";
        let m = CargoParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "cargo");
        assert!(m.name.is_none());
        assert!(m.version.is_none());
        assert!(m.dependencies.is_empty());
    }
}
