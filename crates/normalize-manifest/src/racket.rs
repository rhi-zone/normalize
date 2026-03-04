//! Parser for `info.rkt` files (Racket).
//!
//! Racket package info files use `#lang info` followed by `define` expressions.
//! We heuristically extract `collection`/`name`, `version`, `deps`, and
//! `build-deps` without executing Racket.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `info.rkt` files.
pub struct RacketInfoParser;

impl ManifestParser for RacketInfoParser {
    fn filename(&self) -> &'static str {
        "info.rkt"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // Tokenise the whole file using a minimal S-expression tokeniser.
        // info.rkt is a subset of Racket — each `(define ...)` is a top-level form.
        let tokens: Vec<RktToken> = tokenise_rkt(content);

        for token in &tokens {
            if let RktToken::List(children) = token
                && let Some(RktToken::Atom(head)) = children.first()
                && head == "define"
            {
                parse_define(children, &mut name, &mut version, &mut deps);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "racket",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn parse_define(
    children: &[RktToken],
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    let key = match children.get(1) {
        Some(RktToken::Atom(k)) => k.as_str(),
        _ => return,
    };

    match key {
        "collection" | "name" => {
            if name.is_none()
                && let Some(val) = get_string_value(&children[2..])
            {
                *name = Some(val);
            }
        }
        "version" => {
            if version.is_none()
                && let Some(val) = get_string_value(&children[2..])
            {
                *version = Some(val);
            }
        }
        "deps" => {
            collect_dep_list(&children[2..], DepKind::Normal, deps);
        }
        "build-deps" => {
            collect_dep_list(&children[2..], DepKind::Dev, deps);
        }
        _ => {}
    }
}

/// Get the first string literal from a token slice.
fn get_string_value(tokens: &[RktToken]) -> Option<String> {
    for tok in tokens {
        match tok {
            RktToken::Atom(s) => return Some(strip_string(s)),
            RktToken::List(_) => {}
        }
    }
    None
}

/// Collect deps from the quoted list that follows `(define deps '(...))`.
/// The quote `'` is absorbed by the tokeniser as a `(quote inner-list)` wrapper.
fn collect_dep_list(tokens: &[RktToken], kind: DepKind, deps: &mut Vec<DeclaredDep>) {
    for tok in tokens {
        match tok {
            RktToken::List(inner) => {
                // `'(...)` becomes `(quote (...))` — unwrap the quote.
                if let Some(RktToken::Atom(head)) = inner.first()
                    && head == "quote"
                {
                    // The actual list is the second child.
                    if let Some(RktToken::List(actual)) = inner.get(1) {
                        collect_dep_list_items(actual, kind, deps);
                    }
                    continue;
                }
                // Otherwise treat it as the list of items directly.
                collect_dep_list_items(inner, kind, deps);
            }
            RktToken::Atom(s) => {
                // Bare string atom at the top level after define.
                let s = strip_string(s);
                if !s.is_empty() {
                    deps.push(DeclaredDep {
                        name: s,
                        version_req: None,
                        kind,
                    });
                }
            }
        }
    }
}

fn collect_dep_list_items(items: &[RktToken], kind: DepKind, deps: &mut Vec<DeclaredDep>) {
    for item in items {
        match item {
            RktToken::Atom(s) => {
                // String literals are package names; skip bare symbols like "quote".
                let s_inner = strip_string(s);
                // Only accept values that were originally string literals (started with '"')
                // or bare atoms that look like package names (not empty).
                if !s_inner.is_empty() && (s.trim().starts_with('"') || !s_inner.contains('"')) {
                    deps.push(DeclaredDep {
                        name: s_inner,
                        version_req: None,
                        kind,
                    });
                }
            }
            RktToken::List(sub) => {
                // Check for nested (quote ...) or `("pkg-name" #:version "1.0")`.
                if let Some(RktToken::Atom(head)) = sub.first()
                    && head == "quote"
                {
                    if let Some(RktToken::List(actual)) = sub.get(1) {
                        collect_dep_list_items(actual, kind, deps);
                    }
                    continue;
                }
                if let Some(dep) = parse_versioned_dep(sub, kind) {
                    deps.push(dep);
                }
            }
        }
    }
}

fn parse_versioned_dep(sub: &[RktToken], kind: DepKind) -> Option<DeclaredDep> {
    // Form: ("name" #:version "ver") or ("name" #:version "ver" ...)
    let pkg_name = match sub.first() {
        Some(RktToken::Atom(s)) => strip_string(s),
        _ => return None,
    };
    if pkg_name.is_empty() {
        return None;
    }

    // Look for #:version keyword.
    let mut version_req: Option<String> = None;
    let mut i = 1;
    while i < sub.len() {
        if let RktToken::Atom(kw) = &sub[i]
            && kw == "#:version"
        {
            if let Some(RktToken::Atom(ver)) = sub.get(i + 1) {
                version_req = Some(strip_string(ver));
            }
            break;
        }
        i += 1;
    }

    Some(DeclaredDep {
        name: pkg_name,
        version_req,
        kind,
    })
}

fn strip_string(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// ============================================================================
// Minimal Racket tokeniser (handles strings, atoms, lists, quote)
// ============================================================================

#[derive(Debug, Clone)]
enum RktToken {
    Atom(String),
    List(Vec<RktToken>),
}

fn tokenise_rkt(input: &str) -> Vec<RktToken> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < chars.len() {
        if let Some(tok) = read_rkt_token(&chars, &mut pos) {
            out.push(tok);
        }
    }
    out
}

fn skip_rkt_whitespace_comments(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() {
        match chars[*pos] {
            ' ' | '\t' | '\n' | '\r' => *pos += 1,
            ';' => {
                while *pos < chars.len() && chars[*pos] != '\n' {
                    *pos += 1;
                }
            }
            '#' if *pos + 1 < chars.len() && chars[*pos + 1] == '|' => {
                *pos += 2;
                while *pos + 1 < chars.len() {
                    if chars[*pos] == '|' && chars[*pos + 1] == '#' {
                        *pos += 2;
                        break;
                    }
                    *pos += 1;
                }
            }
            '#' if *pos + 1 < chars.len() && chars[*pos + 1] == '!' => {
                // Shebang or #lang line — skip to end of line.
                while *pos < chars.len() && chars[*pos] != '\n' {
                    *pos += 1;
                }
            }
            _ => break,
        }
    }
}

fn read_rkt_token(chars: &[char], pos: &mut usize) -> Option<RktToken> {
    skip_rkt_whitespace_comments(chars, pos);
    if *pos >= chars.len() {
        return None;
    }

    match chars[*pos] {
        '(' | '[' => {
            let close = if chars[*pos] == '(' { ')' } else { ']' };
            *pos += 1;
            let mut children = Vec::new();
            loop {
                skip_rkt_whitespace_comments(chars, pos);
                if *pos >= chars.len() {
                    break;
                }
                if chars[*pos] == close {
                    *pos += 1;
                    break;
                }
                if let Some(tok) = read_rkt_token(chars, pos) {
                    children.push(tok);
                }
            }
            Some(RktToken::List(children))
        }
        ')' | ']' => {
            *pos += 1;
            None
        }
        '"' => {
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
                *pos += 1;
            }
            Some(RktToken::Atom(s))
        }
        '\'' => {
            // Quote sugar: 'expr → wrap expr in a List.
            *pos += 1;
            read_rkt_token(chars, pos)
                .map(|inner| RktToken::List(vec![RktToken::Atom("quote".to_string()), inner]))
        }
        '`' | ',' => {
            *pos += 1;
            read_rkt_token(chars, pos)
        }
        _ => {
            let mut s = String::new();
            while *pos < chars.len()
                && !matches!(
                    chars[*pos],
                    ' ' | '\t' | '\n' | '\r' | '(' | ')' | '[' | ']' | '"' | ';' | '\''
                )
            {
                s.push(chars[*pos]);
                *pos += 1;
            }
            if s.is_empty() {
                None
            } else {
                Some(RktToken::Atom(s))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#"#lang info
(define collection "my-package")
(define version "1.0.0")
(define deps '("base"
               "racket-lib"
               ("web-server-lib" #:version "1.0")
               ("db-lib" #:version "1.1")))
(define build-deps '("scribble-lib"
                     "racket-doc"))
"#;

    #[test]
    fn test_parse_info_rkt() {
        let m = RacketInfoParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "racket");
        assert_eq!(m.name.as_deref(), Some("my-package"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));

        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"base"), "{names:?}");
        assert!(names.contains(&"racket-lib"), "{names:?}");
        assert!(names.contains(&"web-server-lib"), "{names:?}");
        assert!(names.contains(&"db-lib"), "{names:?}");
        assert!(names.contains(&"scribble-lib"), "{names:?}");
        assert!(names.contains(&"racket-doc"), "{names:?}");

        let web = m
            .dependencies
            .iter()
            .find(|d| d.name == "web-server-lib")
            .unwrap();
        assert_eq!(web.version_req.as_deref(), Some("1.0"));
        assert_eq!(web.kind, DepKind::Normal);

        let scribble = m
            .dependencies
            .iter()
            .find(|d| d.name == "scribble-lib")
            .unwrap();
        assert_eq!(scribble.kind, DepKind::Dev);
    }

    #[test]
    fn test_define_name_fallback() {
        let content = r#"#lang info
(define name "alt-name")
(define version "0.5.0")
(define deps '())
"#;
        let m = RacketInfoParser.parse(content).unwrap();
        assert_eq!(m.name.as_deref(), Some("alt-name"));
    }
}
