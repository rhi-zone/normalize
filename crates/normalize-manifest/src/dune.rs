//! Parser for `dune-project` files (OCaml/Dune).
//!
//! Dune project files use S-expression syntax. We heuristically extract
//! `(package ...)` blocks and parse `(name ...)`, `(version ...)`, and
//! `(depends ...)` from them using the shared [`crate::sexpr`] parser.

use crate::sexpr::Sexp;
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

        for token in &Sexp::parse(content) {
            if let Some(items) = token.tagged_list("package") {
                parse_package_items(items, &mut name, &mut version, &mut deps);
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

fn parse_package_items(
    items: &[Sexp],
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    for item in items {
        let Some(sub) = item.as_list() else { continue };
        let Some(kw) = sub.first().and_then(|t| t.as_atom()) else {
            continue;
        };
        match kw {
            "name" if name.is_none() => {
                if let Some(v) = sub.get(1).and_then(|t| t.as_text()) {
                    *name = Some(v.to_string());
                }
            }
            "version" if version.is_none() => {
                if let Some(v) = sub.get(1).and_then(|t| t.as_text()) {
                    *version = Some(v.to_string());
                }
            }
            "depends" => {
                parse_depends_items(&sub[1..], deps);
            }
            _ => {}
        }
    }
}

fn parse_depends_items(items: &[Sexp], deps: &mut Vec<DeclaredDep>) {
    for item in items {
        match item {
            Sexp::Atom(name) | Sexp::Str(name) => {
                if !name.is_empty() && !name.starts_with(':') {
                    deps.push(DeclaredDep {
                        name: name.clone(),
                        version_req: None,
                        kind: DepKind::Normal,
                    });
                }
            }
            Sexp::List(dep_children) => {
                if let Some(dep) = parse_dep_entry(dep_children) {
                    deps.push(dep);
                }
            }
        }
    }
}

/// Parse a single dependency S-expression like `(pkgname constraints)` or
/// `(pkgname (:with-test) (>= "1.0"))`.
fn parse_dep_entry(children: &[Sexp]) -> Option<DeclaredDep> {
    let name = children.first()?.as_text()?.to_string();
    if name.is_empty() || name.starts_with(':') {
        return None;
    }

    let mut kind = DepKind::Normal;
    let mut version_req: Option<String> = None;

    for token in children.iter().skip(1) {
        match token {
            Sexp::Atom(a) => match a.as_str() {
                ":with-test" | ":with-doc" => kind = DepKind::Dev,
                ":optional" => kind = DepKind::Optional,
                _ => {}
            },
            Sexp::List(constraint) => {
                if let Some(Sexp::Atom(head)) = constraint.first() {
                    match head.as_str() {
                        ":with-test" | ":with-doc" => kind = DepKind::Dev,
                        ":optional" => kind = DepKind::Optional,
                        ">=" | "<=" | ">" | "<" | "=" | "!=" | "~=" => {
                            if let Some(ver) = constraint.get(1).and_then(|t| t.as_text()) {
                                version_req = Some(format!("{head} {ver}"));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Sexp::Str(_) => {}
        }
    }

    Some(DeclaredDep {
        name,
        version_req,
        kind,
    })
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
