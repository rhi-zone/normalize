//! Parser for `*.nimble` files (Nim/Nimble).
//!
//! Nimble files are valid Nim source. We use heuristic line-pattern matching
//! to extract `requires "pkg >= 1.0"` declarations without executing Nim.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `*.nimble` files.
///
/// Since Nimble files use non-standard filenames (e.g. `mypkg.nimble`), this
/// parser is not registered in `parse_manifest()` by filename. Use
/// `parse_manifest_by_extension("nimble", content)` or call `NimbleParser`
/// directly.
pub struct NimbleParser;

impl ManifestParser for NimbleParser {
    fn filename(&self) -> &'static str {
        "*.nimble"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // name = "mypkg"  or  version = "1.0.0"
            if let Some(rest) = trimmed.strip_prefix("name")
                && let Some(val) = extract_assignment_string(rest)
            {
                name = Some(val);
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("version")
                && let Some(val) = extract_assignment_string(rest)
            {
                version = Some(val);
                continue;
            }

            // requires "pkg >= 1.0"  or  requires "pkg"
            if let Some(rest) = trimmed.strip_prefix("requires") {
                // May have multiple on one line: requires "a >= 1", "b"
                extract_requires_strings(rest, &mut deps);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "nimble",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_assignment_string(rest: &str) -> Option<String> {
    let rest = rest.trim().strip_prefix('=')?.trim();
    extract_quoted(rest)
}

fn extract_quoted(s: &str) -> Option<String> {
    let inner = s.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

fn extract_requires_strings(rest: &str, out: &mut Vec<DeclaredDep>) {
    // Extract all quoted strings
    let mut s = rest;
    while let Some(start) = s.find('"') {
        s = &s[start + 1..];
        if let Some(end) = s.find('"') {
            let spec = s[..end].trim();
            if let Some(dep) = parse_nimble_spec(spec) {
                out.push(dep);
            }
            s = &s[end + 1..];
        } else {
            break;
        }
    }
}

fn parse_nimble_spec(spec: &str) -> Option<DeclaredDep> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }

    // Spec forms: "pkg", "pkg >= 1.0", "pkg >= 1.0 & < 2.0"
    const OPS: &[&str] = &[">=", "<=", "!=", ">", "<", "==", "~="];

    for op in OPS {
        if let Some(idx) = spec.find(op) {
            let name = spec[..idx].trim().to_string();
            if name.is_empty() || name == "nim" {
                return None; // Skip the Nim runtime itself
            }
            let version_req = spec[idx..].trim().to_string();
            return Some(DeclaredDep {
                name,
                version_req: Some(version_req),
                kind: DepKind::Normal,
            });
        }
    }

    // Bare name
    if spec == "nim" {
        return None;
    }
    Some(DeclaredDep {
        name: spec.to_string(),
        version_req: None,
        kind: DepKind::Normal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_nimble() {
        let content = r#"# Package
name         = "mypkg"
version      = "0.1.0"
author       = "Alice"
description  = "My package"
license      = "MIT"

# Dependencies
requires "nim >= 1.6.0"
requires "httpclient >= 1.0"
requires "json >= 0.9", "os"
"#;
        let m = NimbleParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "nimble");
        assert_eq!(m.name.as_deref(), Some("mypkg"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));

        // "nim" is filtered out as the runtime
        assert!(!m.dependencies.iter().any(|d| d.name == "nim"));
        assert!(m.dependencies.iter().any(|d| d.name == "httpclient"));
        assert!(m.dependencies.iter().any(|d| d.name == "json"));
        assert!(m.dependencies.iter().any(|d| d.name == "os"));
    }
}
