//! Parsers for Conan C/C++ package manifests.
//!
//! - `conanfile.txt` (v1 INI-style)
//! - `conanfile.py` (v1/v2 Python file, heuristic extraction)

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `conanfile.txt` (Conan v1 INI format).
///
/// Extracts `[requires]` section entries of the form `pkg/version[@user/channel]`.
pub struct ConanTxtParser;

impl ManifestParser for ConanTxtParser {
    fn filename(&self) -> &'static str {
        "conanfile.txt"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let deps = parse_conan_txt(content);
        Ok(ParsedManifest {
            ecosystem: "conan",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

pub(crate) fn parse_conan_txt(content: &str) -> Vec<DeclaredDep> {
    let mut deps = Vec::new();
    let mut in_requires = false;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') {
            // Section header — only [requires] yields deps
            in_requires = line.eq_ignore_ascii_case("[requires]");
            continue;
        }
        if !in_requires || line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Strip inline comment
        let dep_str = line.split('#').next().unwrap_or(line).trim();
        if dep_str.is_empty() {
            continue;
        }

        // Format: pkg/version  or  pkg/version@user/channel
        if let Some(slash_idx) = dep_str.find('/') {
            let name = dep_str[..slash_idx].trim().to_string();
            let rest = dep_str[slash_idx + 1..].trim();
            // Strip @user/channel if present
            let version = rest.split('@').next().unwrap_or(rest).trim();
            let version_req = if version.is_empty() {
                None
            } else {
                Some(version.to_string())
            };
            if !name.is_empty() {
                deps.push(DeclaredDep {
                    name,
                    version_req,
                    kind: DepKind::Normal,
                });
            }
        } else {
            // Bare name with no version
            deps.push(DeclaredDep {
                name: dep_str.to_string(),
                version_req: None,
                kind: DepKind::Normal,
            });
        }
    }

    deps
}

/// Parser for `conanfile.py` (Conan Python file, heuristic).
///
/// Extracts deps from:
/// - `requires = ["pkg/1.0", ...]` list literal
/// - `self.requires("pkg/1.0")` calls
pub struct ConanPyParser;

impl ManifestParser for ConanPyParser {
    fn filename(&self) -> &'static str {
        "conanfile.py"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps = Vec::new();
        let mut in_requires_list = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // requires = ["pkg/1.0", "other/2.0"]  (single or multi-line list)
            if trimmed.starts_with("requires") && trimmed.contains('=') && !in_requires_list {
                // Check if list opens on this line
                let after_eq = trimmed.split_once('=').map(|x| x.1).unwrap_or("").trim();
                extract_quoted_refs(after_eq, &mut deps);
                if after_eq.contains('[') && !after_eq.contains(']') {
                    in_requires_list = true;
                }
                continue;
            }

            if in_requires_list {
                extract_quoted_refs(trimmed, &mut deps);
                if trimmed.contains(']') {
                    in_requires_list = false;
                }
                continue;
            }

            // self.requires("pkg/1.0") or self.requires("pkg/1.0", headers=True)
            if (trimmed.starts_with("self.requires(") || trimmed.starts_with("self.tool_requires("))
                && trimmed.contains('"')
                && let Some(ref_str) = extract_first_string(trimmed)
                && let Some(dep) = conan_ref_to_dep(&ref_str)
            {
                deps.push(dep);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "conan",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

/// Extract all `"pkg/version"` quoted strings from a line and push as deps.
fn extract_quoted_refs(line: &str, out: &mut Vec<DeclaredDep>) {
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('"') {
            let s = &rest[..end];
            if let Some(dep) = conan_ref_to_dep(s) {
                out.push(dep);
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
}

fn extract_first_string(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')?;
    Some(line[start..start + end].to_string())
}

fn conan_ref_to_dep(s: &str) -> Option<DeclaredDep> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // pkg/version@user/channel or pkg/version
    if let Some(slash_idx) = s.find('/') {
        let name = s[..slash_idx].trim().to_string();
        let rest = s[slash_idx + 1..].trim();
        let version = rest.split('@').next().unwrap_or(rest).trim();
        let version_req = if version.is_empty() {
            None
        } else {
            Some(version.to_string())
        };
        if name.is_empty() {
            return None;
        }
        Some(DeclaredDep {
            name,
            version_req,
            kind: DepKind::Normal,
        })
    } else {
        Some(DeclaredDep {
            name: s.to_string(),
            version_req: None,
            kind: DepKind::Normal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_conan_txt() {
        let content = r#"[requires]
zlib/1.2.13
boost/1.83.0@conan/stable
openssl/3.1.0  # pinned

[generators]
cmake
"#;
        let m = ConanTxtParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "conan");
        assert_eq!(m.dependencies.len(), 3);

        let zlib = m.dependencies.iter().find(|d| d.name == "zlib").unwrap();
        assert_eq!(zlib.version_req.as_deref(), Some("1.2.13"));

        let boost = m.dependencies.iter().find(|d| d.name == "boost").unwrap();
        assert_eq!(boost.version_req.as_deref(), Some("1.83.0"));
    }

    #[test]
    fn test_conan_py_list() {
        let content = r#"from conan import ConanFile

class MyConan(ConanFile):
    requires = ["zlib/1.2.13", "boost/1.83.0"]
    tool_requires = []
"#;
        let m = ConanPyParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "conan");
        assert_eq!(m.dependencies.len(), 2);
        assert!(m.dependencies.iter().any(|d| d.name == "zlib"));
        assert!(m.dependencies.iter().any(|d| d.name == "boost"));
    }

    #[test]
    fn test_conan_py_self_requires() {
        let content = r#"from conan import ConanFile

class MyConan(ConanFile):
    def requirements(self):
        self.requires("zlib/1.2.13")
        self.requires("openssl/3.1.0", headers=True)
        self.tool_requires("cmake/3.25.0")
"#;
        let m = ConanPyParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 3);
    }
}
