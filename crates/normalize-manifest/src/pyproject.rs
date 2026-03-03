//! Parser for `pyproject.toml` files (PEP 621 / Poetry).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};
use toml::Value;

/// Parser for `pyproject.toml` files.
///
/// Supports both PEP 621 (`[project.dependencies]`) and
/// Poetry (`[tool.poetry.dependencies]`).
pub struct PyprojectParser;

impl ManifestParser for PyprojectParser {
    fn filename(&self) -> &'static str {
        "pyproject.toml"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let toml: Value = content
            .parse::<Value>()
            .map_err(|e| ManifestError(e.to_string()))?;

        // PEP 621: [project]
        let project = toml.get("project");
        let mut name = project
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let mut version = project
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut deps = Vec::new();

        // PEP 621: [project.dependencies] — array of PEP 508 strings
        if let Some(arr) = project
            .and_then(|p| p.get("dependencies"))
            .and_then(|v| v.as_array())
        {
            for item in arr {
                if let Some(s) = item.as_str()
                    && let Some(dep) = parse_pep508_dep(s)
                {
                    deps.push(dep);
                }
            }
        }

        // Poetry: [tool.poetry]
        let poetry = toml.get("tool").and_then(|t| t.get("poetry"));

        if name.is_none() {
            name = poetry
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
        if version.is_none() {
            version = poetry
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        // [tool.poetry.dependencies] — table of name → version/table
        if let Some(poetry_deps) = poetry
            .and_then(|p| p.get("dependencies"))
            .and_then(|v| v.as_table())
        {
            for (dep_name, val) in poetry_deps {
                if dep_name == "python" {
                    continue; // python version constraint, not a package dep
                }
                let version_req = if let Some(s) = val.as_str() {
                    Some(s.to_string())
                } else if let Some(t) = val.as_table() {
                    t.get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                };
                deps.push(DeclaredDep {
                    name: dep_name.clone(),
                    version_req,
                    kind: DepKind::Normal,
                });
            }
        }

        // [tool.poetry.dev-dependencies]
        if let Some(dev_deps) = poetry
            .and_then(|p| p.get("dev-dependencies"))
            .and_then(|v| v.as_table())
        {
            for (dep_name, val) in dev_deps {
                let version_req = if let Some(s) = val.as_str() {
                    Some(s.to_string())
                } else if let Some(t) = val.as_table() {
                    t.get("version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                };
                deps.push(DeclaredDep {
                    name: dep_name.clone(),
                    version_req,
                    kind: DepKind::Dev,
                });
            }
        }

        Ok(ParsedManifest {
            ecosystem: "python",
            name,
            version,
            dependencies: deps,
        })
    }
}

/// Parse a PEP 508 dependency string (e.g., `"requests>=2.28,<3"`, `"flask"`).
fn parse_pep508_dep(s: &str) -> Option<DeclaredDep> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Strip environment markers after `;`
    let s = match s.find(';') {
        Some(idx) => s[..idx].trim(),
        None => s,
    };
    // Split on version operators
    const OPERATORS: &[&str] = &["===", "~=", "==", "!=", ">=", "<=", ">", "<"];
    for op in OPERATORS {
        if let Some(idx) = s.find(op) {
            let name = s[..idx].trim().to_string();
            if !name.is_empty() {
                return Some(DeclaredDep {
                    name,
                    version_req: Some(s[idx..].trim().to_string()),
                    kind: DepKind::Normal,
                });
            }
        }
    }
    if !s.is_empty() {
        return Some(DeclaredDep {
            name: s.to_string(),
            version_req: None,
            kind: DepKind::Normal,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_pep621() {
        let content = r#"
[project]
name = "my-package"
version = "1.0.0"
dependencies = [
    "requests>=2.28",
    "flask",
    "numpy==1.24.0",
]
"#;
        let m = PyprojectParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "python");
        assert_eq!(m.name.as_deref(), Some("my-package"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));
        assert_eq!(m.dependencies.len(), 3);

        let requests = m
            .dependencies
            .iter()
            .find(|d| d.name == "requests")
            .unwrap();
        assert_eq!(requests.version_req.as_deref(), Some(">=2.28"));
    }

    #[test]
    fn test_poetry() {
        let content = r#"
[tool.poetry]
name = "poetry-app"
version = "0.5.0"

[tool.poetry.dependencies]
python = "^3.9"
requests = "^2.28"
click = "^8.0"

[tool.poetry.dev-dependencies]
pytest = "^7.0"
"#;
        let m = PyprojectParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "python");
        assert_eq!(m.name.as_deref(), Some("poetry-app"));
        assert_eq!(m.version.as_deref(), Some("0.5.0"));

        let normal: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Normal)
            .collect();
        // python is skipped
        assert_eq!(normal.len(), 2);

        let dev: Vec<_> = m
            .dependencies
            .iter()
            .filter(|d| d.kind == DepKind::Dev)
            .collect();
        assert_eq!(dev.len(), 1);
        assert_eq!(dev[0].name, "pytest");
    }
}
