//! Parser for `dune-project` files (OCaml/Dune).
//!
//! Dune project files use S-expression syntax. We heuristically extract
//! `(package ...)` blocks and parse `(name ...)`, `(version ...)`, and
//! `(depends ...)` from them without a full S-expression parser.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `dune-project` files.
pub struct DuneParser;

impl ManifestParser for DuneParser {
    fn filename(&self) -> &'static str {
        "dune-project"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        // We parse the first package block we encounter.
        // For multi-package monorepos, we still return one manifest;
        // name/version come from the first package found.
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // Flatten the content into a single string for easier S-expr scanning.
        // We walk token by token (word or paren-delimited atom).
        parse_sexp_content(content, &mut name, &mut version, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "opam",
            name,
            version,
            dependencies: deps,
        })
    }
}

/// Minimal recursive-descent S-expression tokeniser.
///
/// We don't build a full tree — we just look for the patterns we care about:
/// - `(package (name X) (version Y) (depends ...))`
/// - Inside depends, `(dep-name constraints)` or bare `dep-name`
fn parse_sexp_content(
    content: &str,
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    // Collect top-level tokens (atoms and paren groups as spans).
    // We do a simple depth-tracking scan.
    let tokens: Vec<SexpToken> = tokenise(content);

    // Walk top-level lists looking for (package ...).
    let mut i = 0;
    while i < tokens.len() {
        if let SexpToken::List(children) = &tokens[i]
            && let Some(SexpToken::Atom(head)) = children.first()
            && head == "package"
        {
            parse_package_list(children, name, version, deps);
        }
        i += 1;
    }
}

fn parse_package_list(
    children: &[SexpToken],
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    let mut i = 1; // skip "package" atom
    while i < children.len() {
        if let SexpToken::List(sub) = &children[i]
            && let Some(SexpToken::Atom(kw)) = sub.first()
        {
            match kw.as_str() {
                "name" if name.is_none() => {
                    if let Some(SexpToken::Atom(v)) = sub.get(1) {
                        *name = Some(strip_quotes(v));
                    }
                }
                "version" if version.is_none() => {
                    if let Some(SexpToken::Atom(v)) = sub.get(1) {
                        *version = Some(strip_quotes(v));
                    }
                }
                "depends" => {
                    parse_depends_list(sub, deps);
                }
                _ => {}
            }
        }
        i += 1;
    }
}

fn parse_depends_list(sub: &[SexpToken], deps: &mut Vec<DeclaredDep>) {
    // sub[0] is "depends", rest are dep entries
    let mut i = 1;
    while i < sub.len() {
        match &sub[i] {
            SexpToken::Atom(name) => {
                // bare dep name (possibly a keyword like :with-test at some position)
                let name = strip_quotes(name);
                if !name.is_empty() && !name.starts_with(':') {
                    deps.push(DeclaredDep {
                        name,
                        version_req: None,
                        kind: DepKind::Normal,
                    });
                }
            }
            SexpToken::List(dep_children) => {
                if let Some(dep) = parse_dep_entry(dep_children) {
                    deps.push(dep);
                }
            }
        }
        i += 1;
    }
}

/// Parse a single dependency S-expression like `(pkgname constraints)` or
/// `(pkgname (:with-test) (>= "1.0"))`.
fn parse_dep_entry(children: &[SexpToken]) -> Option<DeclaredDep> {
    let name = match children.first()? {
        SexpToken::Atom(a) => strip_quotes(a),
        _ => return None,
    };
    if name.is_empty() || name.starts_with(':') {
        return None;
    }

    // Determine kind from keyword atoms inside the dep list.
    let mut kind = DepKind::Normal;
    let mut version_req: Option<String> = None;

    for token in children.iter().skip(1) {
        match token {
            SexpToken::Atom(a) => {
                if a == ":with-test" || a == ":with-doc" {
                    kind = DepKind::Dev;
                }
                // bare keyword like :optional
                if a == ":optional" {
                    kind = DepKind::Optional;
                }
            }
            SexpToken::List(constraint) => {
                // Could be (:with-test) or (>= "4.14") or (= "1.0") etc.
                if let Some(SexpToken::Atom(head)) = constraint.first() {
                    match head.as_str() {
                        ":with-test" | ":with-doc" => kind = DepKind::Dev,
                        ":optional" => kind = DepKind::Optional,
                        ">=" | "<=" | ">" | "<" | "=" | "!=" | "~=" => {
                            if let Some(SexpToken::Atom(ver)) = constraint.get(1) {
                                version_req = Some(format!("{} {}", head, strip_quotes(ver)));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Some(DeclaredDep {
        name,
        version_req,
        kind,
    })
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// ============================================================================
// Minimal S-expression tokeniser
// ============================================================================

#[derive(Debug, Clone)]
enum SexpToken {
    Atom(String),
    List(Vec<SexpToken>),
}

fn tokenise(input: &str) -> Vec<SexpToken> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < chars.len() {
        if let Some(tok) = read_token(&chars, &mut pos) {
            out.push(tok);
        }
    }
    out
}

fn skip_whitespace_and_comments(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() {
        if chars[*pos].is_whitespace() {
            *pos += 1;
        } else if chars[*pos] == ';' {
            // line comment
            while *pos < chars.len() && chars[*pos] != '\n' {
                *pos += 1;
            }
        } else {
            break;
        }
    }
}

fn read_token(chars: &[char], pos: &mut usize) -> Option<SexpToken> {
    skip_whitespace_and_comments(chars, pos);
    if *pos >= chars.len() {
        return None;
    }
    match chars[*pos] {
        '(' => {
            *pos += 1;
            let mut children = Vec::new();
            loop {
                skip_whitespace_and_comments(chars, pos);
                if *pos >= chars.len() {
                    break;
                }
                if chars[*pos] == ')' {
                    *pos += 1;
                    break;
                }
                if let Some(tok) = read_token(chars, pos) {
                    children.push(tok);
                }
            }
            Some(SexpToken::List(children))
        }
        ')' => {
            // Unexpected closing paren — skip
            *pos += 1;
            None
        }
        '"' => {
            // Quoted string
            *pos += 1;
            let mut s = String::from('"');
            while *pos < chars.len() && chars[*pos] != '"' {
                if chars[*pos] == '\\' {
                    *pos += 1;
                }
                if *pos < chars.len() {
                    s.push(chars[*pos]);
                    *pos += 1;
                }
            }
            s.push('"');
            if *pos < chars.len() {
                *pos += 1; // closing "
            }
            Some(SexpToken::Atom(s))
        }
        _ => {
            // Bare atom: read until whitespace or paren
            let mut s = String::new();
            while *pos < chars.len()
                && !chars[*pos].is_whitespace()
                && chars[*pos] != '('
                && chars[*pos] != ')'
                && chars[*pos] != '"'
                && chars[*pos] != ';'
            {
                s.push(chars[*pos]);
                *pos += 1;
            }
            if s.is_empty() {
                None
            } else {
                Some(SexpToken::Atom(s))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#"(lang dune 3.0)

(name my-project)

(package
 (name my-package)
 (version "0.1.0")
 (depends
  (ocaml (>= "4.14"))
  (dune (>= "3.0"))
  (cmdliner (>= "1.1"))
  (alcotest :with-test)))
"#;

    #[test]
    fn test_parse_dune_project() {
        let m = DuneParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "opam");
        assert_eq!(m.name.as_deref(), Some("my-package"));
        assert_eq!(m.version.as_deref(), Some("0.1.0"));

        let dep_names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(
            dep_names.contains(&"ocaml"),
            "expected ocaml in {dep_names:?}"
        );
        assert!(
            dep_names.contains(&"dune"),
            "expected dune in {dep_names:?}"
        );
        assert!(
            dep_names.contains(&"cmdliner"),
            "expected cmdliner in {dep_names:?}"
        );
        assert!(
            dep_names.contains(&"alcotest"),
            "expected alcotest in {dep_names:?}"
        );

        let ocaml = m.dependencies.iter().find(|d| d.name == "ocaml").unwrap();
        assert_eq!(ocaml.version_req.as_deref(), Some(">= 4.14"));
        assert_eq!(ocaml.kind, DepKind::Normal);

        let alcotest = m
            .dependencies
            .iter()
            .find(|d| d.name == "alcotest")
            .unwrap();
        assert_eq!(alcotest.kind, DepKind::Dev);
    }

    #[test]
    fn test_bare_dep_name() {
        let content = r#"(lang dune 3.0)
(package
 (name pkg)
 (depends
  fmt
  logs))
"#;
        let m = DuneParser.parse(content).unwrap();
        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"fmt"), "{names:?}");
        assert!(names.contains(&"logs"), "{names:?}");
    }
}
