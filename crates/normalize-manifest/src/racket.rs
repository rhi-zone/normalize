//! Parser for `info.rkt` files (Racket).
//!
//! Racket package info files use `#lang info` followed by `define` expressions.
//! We heuristically extract `collection`/`name`, `version`, `deps`, and
//! `build-deps` without executing Racket, using the shared [`crate::sexpr`] parser.

use crate::sexpr::{Sexp, kw_arg};
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

        for token in &Sexp::parse(content) {
            if let Some(items) = token.tagged_list("define") {
                parse_define(items, &mut name, &mut version, &mut deps);
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
    items: &[Sexp],
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    let key = match items.first() {
        Some(Sexp::Atom(k)) => k.as_str(),
        _ => return,
    };

    match key {
        "collection" | "name" => {
            if name.is_none()
                && let Some(val) = first_text_value(&items[1..])
            {
                *name = Some(val);
            }
        }
        "version" => {
            if version.is_none()
                && let Some(val) = first_text_value(&items[1..])
            {
                *version = Some(val);
            }
        }
        "deps" => {
            collect_dep_list(&items[1..], DepKind::Normal, deps);
        }
        "build-deps" => {
            collect_dep_list(&items[1..], DepKind::Dev, deps);
        }
        _ => {}
    }
}

/// Return the first string or atom value from a token slice.
fn first_text_value(tokens: &[Sexp]) -> Option<String> {
    for tok in tokens {
        if let Some(s) = tok.as_text() {
            return Some(s.to_string());
        }
    }
    None
}

/// Collect deps from the quoted list that follows `(define deps '(...))`.
/// The `'` is parsed as `(quote inner-list)` by the shared parser.
fn collect_dep_list(tokens: &[Sexp], kind: DepKind, deps: &mut Vec<DeclaredDep>) {
    for tok in tokens {
        match tok {
            Sexp::List(inner) => {
                // `'(...)` became `(quote (...))` — unwrap the quote.
                if let Some(Sexp::Atom(head)) = inner.first()
                    && head == "quote"
                {
                    if let Some(Sexp::List(actual)) = inner.get(1) {
                        collect_dep_list_items(actual, kind, deps);
                    }
                    continue;
                }
                // Otherwise treat it as the list of items directly.
                collect_dep_list_items(inner, kind, deps);
            }
            Sexp::Str(s) | Sexp::Atom(s) => {
                if !s.is_empty() {
                    deps.push(DeclaredDep {
                        name: s.clone(),
                        version_req: None,
                        kind,
                    });
                }
            }
        }
    }
}

fn collect_dep_list_items(items: &[Sexp], kind: DepKind, deps: &mut Vec<DeclaredDep>) {
    for item in items {
        match item {
            Sexp::Str(s) => {
                // String literals are package names.
                if !s.is_empty() {
                    deps.push(DeclaredDep {
                        name: s.clone(),
                        version_req: None,
                        kind,
                    });
                }
            }
            Sexp::Atom(s) => {
                // Bare symbols are also package names; skip keywords.
                if !s.is_empty() && !s.starts_with('#') && !s.starts_with(':') {
                    deps.push(DeclaredDep {
                        name: s.clone(),
                        version_req: None,
                        kind,
                    });
                }
            }
            Sexp::List(sub) => {
                // Check for nested `(quote ...)`.
                if let Some(Sexp::Atom(head)) = sub.first()
                    && head == "quote"
                {
                    if let Some(Sexp::List(actual)) = sub.get(1) {
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

fn parse_versioned_dep(sub: &[Sexp], kind: DepKind) -> Option<DeclaredDep> {
    // Form: ("name" #:version "ver") or ("name" #:version "ver" ...)
    let pkg_name = sub.first()?.as_text()?.to_string();
    if pkg_name.is_empty() {
        return None;
    }

    let version_req = kw_arg(sub, "#:version")
        .and_then(|t| t.as_text())
        .map(|s| s.to_string());

    Some(DeclaredDep {
        name: pkg_name,
        version_req,
        kind,
    })
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
