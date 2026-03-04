//! Parser for `cpanfile` files (Perl/CPAN).
//!
//! Heuristic line-based parsing:
//! - `requires 'Pkg'` / `requires 'Pkg', '>= 1.0'` → `DepKind::Normal`
//! - `recommends 'Pkg'` → `DepKind::Optional`
//! - `on 'test' => sub { ... }` / `on 'develop' => sub { ... }` → `DepKind::Dev`
//! - Tracks the current `on` block context.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `cpanfile` files.
pub struct CpanfileParser;

impl ManifestParser for CpanfileParser {
    fn filename(&self) -> &'static str {
        "cpanfile"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // Track context: None = top-level, Some(kind) = inside an on block
        let mut block_kind: Option<DepKind> = None;
        let mut brace_depth: i32 = 0;
        // Have we opened the on-block yet?
        let mut in_on_header = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Count braces to track block depth
            for ch in trimmed.chars() {
                match ch {
                    '{' => {
                        brace_depth += 1;
                        if in_on_header {
                            in_on_header = false;
                        }
                    }
                    '}' => {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            block_kind = None;
                        }
                    }
                    _ => {}
                }
            }

            // `on 'test' => sub {` or `on 'develop' => sub {`
            if trimmed.starts_with("on ") || trimmed.starts_with("on\t") {
                block_kind = Some(parse_on_kind(trimmed));
                in_on_header = true;
                continue;
            }

            // `requires 'Pkg'` / `requires 'Pkg', '>= 1.0';`
            if trimmed.starts_with("requires ") || trimmed.starts_with("requires\t") {
                let kind = block_kind.unwrap_or(DepKind::Normal);
                if let Some(dep) = parse_cpan_dep_line(trimmed, kind) {
                    deps.push(dep);
                }
                continue;
            }

            // `recommends 'Pkg'`
            if (trimmed.starts_with("recommends ") || trimmed.starts_with("recommends\t"))
                && let Some(dep) = parse_cpan_dep_line(trimmed, DepKind::Optional)
            {
                deps.push(dep);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "cpan",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

/// Determine `DepKind` from `on 'test' => sub {` line.
fn parse_on_kind(line: &str) -> DepKind {
    if line.contains("'test'")
        || line.contains("\"test\"")
        || line.contains("'develop'")
        || line.contains("\"develop\"")
    {
        DepKind::Dev
    } else {
        DepKind::Optional
    }
}

/// Parse `requires 'Pkg', '>= 1.0';` or `recommends 'Pkg';`.
fn parse_cpan_dep_line(line: &str, kind: DepKind) -> Option<DeclaredDep> {
    // Strip verb keyword
    let rest = line
        .trim_start_matches("requires")
        .trim_start_matches("recommends")
        .trim();

    // Collect all single- or double-quoted strings
    let quoted = extract_quoted_strings(rest);
    if quoted.is_empty() {
        return None;
    }

    let name = quoted[0].clone();
    if name.is_empty() || name == "perl" {
        return None; // Skip the perl runtime itself
    }

    let version_req = quoted.get(1).cloned().filter(|v| !v.is_empty());

    Some(DeclaredDep {
        name,
        version_req,
        kind,
    })
}

fn extract_quoted_strings(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' || ch == '"' {
            let mut token = String::new();
            for inner in chars.by_ref() {
                if inner == ch {
                    break;
                }
                token.push(inner);
            }
            result.push(token);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_cpanfile() {
        let content = r#"requires 'perl', '5.10.0';
requires 'Moose', '>= 2.0';
requires 'namespace::autoclean';
recommends 'DateTime';
on 'test' => sub {
    requires 'Test::More', '>= 0.98';
    requires 'Test::Exception';
};
on 'develop' => sub {
    requires 'Dist::Zilla';
    requires 'Pod::Coverage';
};
"#;
        let m = CpanfileParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "cpan");

        let moose = m.dependencies.iter().find(|d| d.name == "Moose").unwrap();
        assert_eq!(moose.kind, DepKind::Normal);
        assert_eq!(moose.version_req.as_deref(), Some(">= 2.0"));

        let ns = m
            .dependencies
            .iter()
            .find(|d| d.name == "namespace::autoclean")
            .unwrap();
        assert_eq!(ns.kind, DepKind::Normal);
        assert!(ns.version_req.is_none());

        let dt = m
            .dependencies
            .iter()
            .find(|d| d.name == "DateTime")
            .unwrap();
        assert_eq!(dt.kind, DepKind::Optional);

        let tm = m
            .dependencies
            .iter()
            .find(|d| d.name == "Test::More")
            .unwrap();
        assert_eq!(tm.kind, DepKind::Dev);
        assert_eq!(tm.version_req.as_deref(), Some(">= 0.98"));

        let dz = m
            .dependencies
            .iter()
            .find(|d| d.name == "Dist::Zilla")
            .unwrap();
        assert_eq!(dz.kind, DepKind::Dev);

        // perl runtime should be skipped
        assert!(!m.dependencies.iter().any(|d| d.name == "perl"));
    }

    #[test]
    fn test_nested_on_blocks() {
        let content =
            "requires 'Scalar::Util';\non 'test' => sub {\n    requires 'Test::Deep';\n};\n";
        let m = CpanfileParser.parse(content).unwrap();
        let su = m
            .dependencies
            .iter()
            .find(|d| d.name == "Scalar::Util")
            .unwrap();
        assert_eq!(su.kind, DepKind::Normal);
        let td = m
            .dependencies
            .iter()
            .find(|d| d.name == "Test::Deep")
            .unwrap();
        assert_eq!(td.kind, DepKind::Dev);
    }
}
