//! Parser for `fpm.toml` files (Fortran Package Manager).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use toml::Value;

/// Parser for `fpm.toml` files.
///
/// `[dependencies]` → `Normal`, `[dev-dependencies]` → `Dev`.
/// Dependencies are tables with optional `tag`, `git`, or `path` keys.
/// Version is taken from `tag` if present, otherwise `None`.
pub struct FortranFpmParser;

impl ManifestParser for FortranFpmParser {
    fn filename(&self) -> &'static str {
        "fpm.toml"
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
        extract_fpm_deps(&toml, "dependencies", DepKind::Normal, &mut deps);
        extract_fpm_deps(&toml, "dev-dependencies", DepKind::Dev, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "fpm",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_fpm_deps(toml: &Value, section: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    let Some(table) = toml.get(section).and_then(|v| v.as_table()) else {
        return;
    };
    for (name, val) in table {
        let version_req = val
            .as_table()
            .and_then(|t| t.get("tag"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
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
    fn test_fpm_toml() {
        let content = r#"
name = "my_lib"
version = "0.1.0"

[dependencies]
fortran-stdlib = { git = "https://github.com/fortran-lang/stdlib" }
M_math = { git = "https://github.com/urbanjost/M_math.git", tag = "v0.0.1" }
toml-f = { path = "../toml-f" }

[dev-dependencies]
test-drive = { git = "https://github.com/fortran-lang/test-drive.git" }
"#;
        let m = FortranFpmParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "fpm");
        assert_eq!(m.name.as_deref(), Some("my_lib"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));
        assert_eq!(m.dependencies.len(), 4);

        let stdlib = m
            .dependencies
            .iter()
            .find(|d| d.name == "fortran-stdlib")
            .unwrap();
        assert_eq!(stdlib.kind, DepKind::Normal);
        assert!(stdlib.version_req.is_none());

        let mmath = m.dependencies.iter().find(|d| d.name == "M_math").unwrap();
        assert_eq!(mmath.kind, DepKind::Normal);
        assert_eq!(mmath.version_req.as_deref(), Some("v0.0.1"));

        let tomlf = m.dependencies.iter().find(|d| d.name == "toml-f").unwrap();
        assert_eq!(tomlf.kind, DepKind::Normal);
        assert!(tomlf.version_req.is_none());

        let test_drive = m
            .dependencies
            .iter()
            .find(|d| d.name == "test-drive")
            .unwrap();
        assert_eq!(test_drive.kind, DepKind::Dev);
        assert!(test_drive.version_req.is_none());
    }

    #[test]
    fn test_fpm_no_dev_deps() {
        let content = r#"
name = "minimal"
version = "0.0.1"

[dependencies]
stdlib = { git = "https://github.com/fortran-lang/stdlib", tag = "v0.4.0" }
"#;
        let m = FortranFpmParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].version_req.as_deref(), Some("v0.4.0"));
    }
}
