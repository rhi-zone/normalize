//! Parser for `*.cabal` files (Haskell/Cabal).
//!
//! Heuristic extraction of `build-depends:` fields using line-pattern matching.
//! Handles multi-line `build-depends` lists with comma-separated entries.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `*.cabal` files.
///
/// Since cabal files use non-standard filenames (e.g. `mypkg.cabal`), this
/// parser is not registered in `parse_manifest()` by filename. Use
/// `parse_manifest_by_extension("cabal", content)` or call `CabalParser` directly.
pub struct CabalParser;

impl ManifestParser for CabalParser {
    fn filename(&self) -> &'static str {
        "*.cabal"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name = None;
        let mut version = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();
        let mut in_build_depends = false;
        let mut is_test = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                in_build_depends = false;
                continue;
            }
            if trimmed.starts_with("--") {
                continue;
            }

            let lower = trimmed.to_ascii_lowercase();

            // Package-level name / version
            if lower.starts_with("name:") && name.is_none() {
                name = Some(trimmed["name:".len()..].trim().to_string());
                continue;
            }
            if lower.starts_with("version:") && version.is_none() {
                version = Some(trimmed["version:".len()..].trim().to_string());
                continue;
            }

            // Detect component type (test-suite → Dev)
            if lower.starts_with("test-suite") || lower.starts_with("benchmark") {
                is_test = true;
            }
            if lower.starts_with("library") || lower.starts_with("executable") {
                is_test = false;
            }

            // build-depends: pkg1 >= 1.0, pkg2
            if lower.starts_with("build-depends:") {
                in_build_depends = true;
                let rest = &trimmed["build-depends:".len()..];
                extract_cabal_deps(rest, is_test, &mut deps);
                continue;
            }

            if in_build_depends {
                // Continuation: must be indented (or start with comma)
                if line.starts_with([' ', '\t']) || trimmed.starts_with(',') {
                    extract_cabal_deps(trimmed, is_test, &mut deps);
                } else {
                    in_build_depends = false;
                }
            }
        }

        // Deduplicate (same dep may appear in both library and test-suite)
        deps.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

        Ok(ParsedManifest {
            ecosystem: "cabal",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn extract_cabal_deps(line: &str, is_test: bool, out: &mut Vec<DeclaredDep>) {
    let kind = if is_test {
        DepKind::Dev
    } else {
        DepKind::Normal
    };

    for part in line.split(',') {
        let part = part.trim().trim_start_matches(',').trim();
        if part.is_empty() || part.starts_with("--") {
            continue;
        }

        // Format: `pkg-name >= 1.0 && < 2.0`  or  `pkg-name`
        // Package name is the first token (may contain hyphens and dots)
        let mut tokens = part.splitn(2, ['>', '<', '=', '&', '!']);
        let name_part = tokens.next().unwrap_or("").trim();

        // Strip trailing version-constraint characters from name
        let name = name_part
            .trim_end_matches(['>', '<', '=', '~', ' '])
            .to_string();

        if name.is_empty() || name == "base" {
            // `base` is the Haskell Prelude — always present, not a real dependency
            continue;
        }

        // Extract version constraint: everything after the package name
        let version_req = part
            .find(['>', '<', '='])
            .map(|idx| part[idx..].trim().to_string());

        out.push(DeclaredDep {
            name,
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
    fn test_parse_cabal() {
        let content = r#"cabal-version: 2.4
name:          my-package
version:       0.1.0.0
license:       MIT

library
  exposed-modules: MyLib
  build-depends:
    base           >= 4.14 && < 5,
    text           >= 1.2  && < 2.1,
    aeson          >= 2.0

test-suite my-test
  type:         exitcode-stdio-1.0
  build-depends:
    base,
    hspec >= 2.11
"#;
        let m = CabalParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "cabal");
        assert_eq!(m.name.as_deref(), Some("my-package"));
        assert_eq!(m.version.as_deref(), Some("0.1.0.0"));

        // base is filtered
        assert!(!m.dependencies.iter().any(|d| d.name == "base"));

        let text = m.dependencies.iter().find(|d| d.name == "text").unwrap();
        assert_eq!(text.kind, DepKind::Normal);

        let hspec = m.dependencies.iter().find(|d| d.name == "hspec").unwrap();
        assert_eq!(hspec.kind, DepKind::Dev);
    }
}
