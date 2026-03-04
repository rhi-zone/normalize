//! Parser for `*.opam` files (OCaml/OPAM).
//!
//! Heuristic line-based parsing of OPAM package files:
//! - `name:` / `version:` → package metadata
//! - `depends:` list → `DepKind::Normal` by default,
//!   `{with-test ...}` constraint → `DepKind::Dev`
//! - `depopts:` list → `DepKind::Optional`
//!
//! OPAM list format:
//! ```text
//! depends: [
//!   "pkg-name" {>= "1.0"}
//!   "other-pkg"
//! ]
//! ```

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `*.opam` files (OCaml OPAM packages).
///
/// Since OPAM files use non-standard filenames (e.g. `mypackage.opam`),
/// register via extension using `parse_manifest_by_extension("opam", content)`.
pub struct OpamParser;

impl ManifestParser for OpamParser {
    fn filename(&self) -> &'static str {
        "*.opam"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        #[derive(Clone, Copy, PartialEq)]
        enum Section {
            None,
            Depends,
            Depopts,
        }

        let mut section = Section::None;
        let mut bracket_depth: i32 = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Check for section headers at depth 0
            if bracket_depth == 0 {
                if let Some(val) = parse_opam_field(trimmed, "name") {
                    name = Some(val);
                    continue;
                }
                if let Some(val) = parse_opam_field(trimmed, "version") {
                    version = Some(val);
                    continue;
                }

                // `depends: [` or `depends: [ ... ]`
                if trimmed.starts_with("depends:") {
                    section = Section::Depends;
                } else if trimmed.starts_with("depopts:") {
                    section = Section::Depopts;
                } else if !trimmed.starts_with('"') {
                    // Non-dep line at top level — could be another field starting a new block
                    if !trimmed.contains('[') {
                        section = Section::None;
                    }
                }
            }

            // Count brackets
            for ch in trimmed.chars() {
                match ch {
                    '[' => bracket_depth += 1,
                    ']' => {
                        bracket_depth -= 1;
                        if bracket_depth == 0 {
                            section = Section::None;
                        }
                    }
                    _ => {}
                }
            }

            // Parse dep entries inside a section
            if section != Section::None && bracket_depth > 0 {
                // A dep entry looks like: `"pkg-name" {constraints}` or just `"pkg-name"`
                if trimmed.starts_with('"') {
                    let kind_for_section = match section {
                        Section::Depends => DepKind::Normal,
                        Section::Depopts => DepKind::Optional,
                        Section::None => continue,
                    };
                    if let Some(dep) = parse_opam_dep_entry(trimmed, kind_for_section) {
                        deps.push(dep);
                    }
                }
            }
        }

        Ok(ParsedManifest {
            ecosystem: "opam",
            name,
            version,
            dependencies: deps,
        })
    }
}

/// Parse `key: "value"` → `Some(value)` (strips quotes).
fn parse_opam_field(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{}:", key);
    let rest = line.strip_prefix(&prefix)?.trim();
    let val = rest.trim_matches('"');
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

/// Parse a dep entry like `"pkg-name" {>= "1.0"}` or `"pkg-name" {with-test & >= "1.6"}`.
fn parse_opam_dep_entry(line: &str, default_kind: DepKind) -> Option<DeclaredDep> {
    // Extract the package name (first quoted string)
    let after_quote = line.strip_prefix('"')?;
    let name_end = after_quote.find('"')?;
    let pkg_name = after_quote[..name_end].to_string();
    if pkg_name.is_empty() {
        return None;
    }

    let rest = after_quote[name_end + 1..].trim();

    // No constraints
    if rest.is_empty() || !rest.contains('{') {
        return Some(DeclaredDep {
            name: pkg_name,
            version_req: None,
            kind: default_kind,
        });
    }

    // Parse constraint block `{...}`
    let brace_start = rest.find('{')?;
    let brace_content_start = brace_start + 1;
    let brace_end = rest.rfind('}')?;
    let constraint = rest[brace_content_start..brace_end].trim();

    // Detect `with-test` → Dev
    let kind = if constraint.contains("with-test") || constraint.contains("with-doc") {
        DepKind::Dev
    } else {
        default_kind
    };

    // Extract version: look for `>= "x"` or `"x"` patterns within constraint
    let version_req = extract_opam_version(constraint);

    Some(DeclaredDep {
        name: pkg_name,
        version_req,
        kind,
    })
}

/// Extract a version string from an OPAM constraint like `>= "4.14"` or `>= "1.0" & < "2.0"`.
fn extract_opam_version(constraint: &str) -> Option<String> {
    // Remove with-test and similar flags, collect version constraints
    let mut parts = Vec::new();

    let clean = constraint
        .replace("with-test", "")
        .replace("with-doc", "")
        .replace(['&', '|'], " ");

    let mut chars = clean.chars().peekable();
    let mut current_op = String::new();

    while let Some(ch) = chars.next() {
        match ch {
            '>' | '<' | '=' | '!' => {
                current_op.push(ch);
                // Collect the rest of the operator
                while chars.peek().is_some_and(|&c| matches!(c, '>' | '<' | '=')) {
                    current_op.push(chars.next().unwrap());
                }
                // Skip whitespace
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
                // Read quoted version
                if chars.peek() == Some(&'"') {
                    chars.next(); // consume '"'
                    let mut ver = String::new();
                    for c in chars.by_ref() {
                        if c == '"' {
                            break;
                        }
                        ver.push(c);
                    }
                    parts.push(format!("{} \"{}\"", current_op.trim(), ver));
                    current_op.clear();
                }
            }
            '"' => {
                // Bare quoted version (no operator)
                let mut ver = String::new();
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    ver.push(c);
                }
                if !ver.is_empty() {
                    parts.push(format!("\"{}\"", ver));
                }
            }
            _ => {}
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" & "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_opam() {
        let content = r#"opam-version: "2.0"
name: "my-package"
version: "0.1.0"
synopsis: "My OCaml package"
depends: [
  "ocaml" {>= "4.14"}
  "dune" {>= "3.0"}
  "cmdliner" {>= "1.1"}
  "alcotest" {with-test & >= "1.6"}
]
depopts: [
  "ppx_sexp_conv"
]
"#;
        let m = OpamParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "opam");
        assert_eq!(m.name.as_deref(), Some("my-package"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));

        let ocaml = m.dependencies.iter().find(|d| d.name == "ocaml").unwrap();
        assert_eq!(ocaml.kind, DepKind::Normal);
        assert!(ocaml.version_req.is_some());

        let dune = m.dependencies.iter().find(|d| d.name == "dune").unwrap();
        assert_eq!(dune.kind, DepKind::Normal);

        let alcotest = m
            .dependencies
            .iter()
            .find(|d| d.name == "alcotest")
            .unwrap();
        assert_eq!(alcotest.kind, DepKind::Dev);

        let ppx = m
            .dependencies
            .iter()
            .find(|d| d.name == "ppx_sexp_conv")
            .unwrap();
        assert_eq!(ppx.kind, DepKind::Optional);
    }

    #[test]
    fn test_bare_dep() {
        let content = "opam-version: \"2.0\"\nname: \"mypkg\"\nversion: \"1.0\"\ndepends: [\n  \"ocaml\"\n  \"dune\"\n]\n";
        let m = OpamParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 2);
        assert!(m.dependencies.iter().all(|d| d.kind == DepKind::Normal));
    }
}
