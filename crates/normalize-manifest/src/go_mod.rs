//! Parser for `go.mod` files (Go modules).

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Information extracted from a go.mod file.
#[derive(Debug, Clone)]
pub struct GoModule {
    /// Module path (e.g., `"github.com/user/project"`).
    pub path: String,
    /// Go version directive (e.g., `"1.21"`).
    pub go_version: Option<String>,
}

/// Parse go.mod content to extract module information.
///
/// Exposed via `crate::go_module()` for `normalize-local-deps`.
pub(crate) fn parse_go_module(content: &str) -> Option<GoModule> {
    let mut path = None;
    let mut go_version = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("module ") {
            path = Some(line.trim_start_matches("module ").trim().to_string());
        }
        if line.starts_with("go ") {
            go_version = Some(line.trim_start_matches("go ").trim().to_string());
        }
    }

    path.map(|path| GoModule { path, go_version })
}

/// Parser for `go.mod` files.
pub struct GoModParser;

impl ManifestParser for GoModParser {
    fn filename(&self) -> &'static str {
        "go.mod"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let module = parse_go_module(content)
            .ok_or_else(|| ManifestError("no module directive found".to_string()))?;

        let mut deps = Vec::new();
        let mut in_require_block = false;

        for line in content.lines() {
            let line = line.trim();

            if line == "require (" {
                in_require_block = true;
                continue;
            }
            if in_require_block && line == ")" {
                in_require_block = false;
                continue;
            }
            if in_require_block {
                if let Some(dep) = parse_require_line(line) {
                    deps.push(dep);
                }
            } else if line.starts_with("require ") && !line.contains('(') {
                // Single-line require: `require github.com/foo/bar v1.2.3`
                let rest = line.trim_start_matches("require ").trim();
                if let Some(dep) = parse_require_line(rest) {
                    deps.push(dep);
                }
            }
        }

        Ok(ParsedManifest {
            ecosystem: "go",
            name: Some(module.path),
            version: module.go_version,
            dependencies: deps,
        })
    }
}

fn parse_require_line(line: &str) -> Option<DeclaredDep> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") {
        return None;
    }
    // Strip inline comment: `github.com/foo/bar v1.2.3 // indirect`
    let without_comment = match line.find(" // ") {
        Some(idx) => &line[..idx],
        None => line,
    };
    let mut parts = without_comment.split_whitespace();
    let name = parts.next()?.to_string();
    let version_req = parts.next().map(|v| v.to_string());
    Some(DeclaredDep {
        name,
        version_req,
        kind: DepKind::Normal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_module_and_version() {
        let content = "module github.com/user/project\n\ngo 1.21\n";
        let m = GoModParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "go");
        assert_eq!(m.name.as_deref(), Some("github.com/user/project"));
        assert_eq!(m.version.as_deref(), Some("1.21"));
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_require_block() {
        let content = r#"module github.com/user/project

go 1.21

require (
    github.com/pkg/errors v0.9.1
    golang.org/x/sync v0.3.0 // indirect
)
"#;
        let m = GoModParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 2);
        assert_eq!(m.dependencies[0].name, "github.com/pkg/errors");
        assert_eq!(m.dependencies[0].version_req.as_deref(), Some("v0.9.1"));
        assert_eq!(m.dependencies[1].name, "golang.org/x/sync");
        assert_eq!(m.dependencies[1].version_req.as_deref(), Some("v0.3.0"));
    }

    #[test]
    fn test_single_line_require() {
        let content = "module example.com/m\ngo 1.20\nrequire github.com/foo/bar v1.2.3\n";
        let m = GoModParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "github.com/foo/bar");
        assert_eq!(m.dependencies[0].version_req.as_deref(), Some("v1.2.3"));
    }

    #[test]
    fn test_parse_go_module_helper() {
        let content = "module mymod\ngo 1.22\n";
        let gm = parse_go_module(content).unwrap();
        assert_eq!(gm.path, "mymod");
        assert_eq!(gm.go_version.as_deref(), Some("1.22"));
    }
}
