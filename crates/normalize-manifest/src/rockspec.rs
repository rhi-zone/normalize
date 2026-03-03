//! Parser for `*.rockspec` files (Lua/LuaRocks).
//!
//! Rockspec files are Lua source. We heuristically extract the `dependencies`
//! table (a list of strings) without executing Lua.
//!
//! Dependency string format: `"pkg >= 1.0"` or `"pkg"`.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `*.rockspec` files.
///
/// Since rockspec files use non-standard filenames that include the version
/// (e.g. `mypkg-1.0-1.rockspec`), this parser is not registered in
/// `parse_manifest()` by filename. Use `parse_manifest_by_extension("rockspec", content)`
/// or call `RockspecParser` directly.
pub struct RockspecParser;

impl ManifestParser for RockspecParser {
    fn filename(&self) -> &'static str {
        "*.rockspec"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps = Vec::new();
        let mut in_deps = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }

            // package = "mypkg"
            if trimmed.starts_with("package")
                && trimmed.contains('=')
                && name.is_none()
                && let Some(v) = extract_lua_string(trimmed)
            {
                name = Some(v);
                continue;
            }

            // version = "1.0-1"
            if trimmed.starts_with("version")
                && trimmed.contains('=')
                && version.is_none()
                && let Some(v) = extract_lua_string(trimmed)
            {
                version = Some(v);
                continue;
            }

            // dependencies = { "lua >= 5.1", "pkg ~> 1.0" }
            if trimmed.starts_with("dependencies") && trimmed.contains('{') {
                in_deps = true;
                extract_dep_strings(trimmed, &mut deps);
                if trimmed.contains('}') {
                    in_deps = false;
                }
                continue;
            }

            if in_deps {
                extract_dep_strings(trimmed, &mut deps);
                if trimmed.contains('}') {
                    in_deps = false;
                }
            }
        }

        Ok(ParsedManifest {
            ecosystem: "luarocks",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_lua_string(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')?;
    Some(line[start..start + end].to_string())
}

fn extract_dep_strings(line: &str, out: &mut Vec<DeclaredDep>) {
    let mut s = line;
    while let Some(q_start) = s.find('"') {
        s = &s[q_start + 1..];
        if let Some(q_end) = s.find('"') {
            let spec = s[..q_end].trim();
            if let Some(dep) = parse_rockspec_spec(spec) {
                out.push(dep);
            }
            s = &s[q_end + 1..];
        } else {
            break;
        }
    }
}

fn parse_rockspec_spec(spec: &str) -> Option<DeclaredDep> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }

    const OPS: &[&str] = &[">=", "<=", "!=", ">", "<", "==", "~>"];
    for op in OPS {
        if let Some(idx) = spec.find(op) {
            let name = spec[..idx].trim().to_string();
            if name.is_empty() || name == "lua" {
                return None; // Skip the Lua runtime itself
            }
            let version_req = spec[idx..].trim().to_string();
            return Some(DeclaredDep {
                name,
                version_req: Some(version_req),
                kind: DepKind::Normal,
            });
        }
    }

    if spec == "lua" {
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
    fn test_parse_rockspec() {
        let content = r#"package = "mypkg"
version = "1.0-1"
source = { url = "https://example.com/mypkg-1.0.tar.gz" }
description = { summary = "My package" }

dependencies = {
  "lua >= 5.1",
  "luasocket >= 3.0",
  "dkjson ~> 2.5",
  "argparse"
}

build = { type = "builtin" }
"#;
        let m = RockspecParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "luarocks");
        assert_eq!(m.name.as_deref(), Some("mypkg"));
        assert_eq!(m.version.as_deref(), Some("1.0-1"));

        // lua is filtered
        assert!(!m.dependencies.iter().any(|d| d.name == "lua"));

        let socket = m
            .dependencies
            .iter()
            .find(|d| d.name == "luasocket")
            .unwrap();
        assert_eq!(socket.version_req.as_deref(), Some(">= 3.0"));

        let argparse = m
            .dependencies
            .iter()
            .find(|d| d.name == "argparse")
            .unwrap();
        assert!(argparse.version_req.is_none());
    }
}
