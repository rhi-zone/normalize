//! Parser for `*.asd` files (Common Lisp/ASDF).
//!
//! ASDF system definition files use Common Lisp syntax. We heuristically
//! extract `(asdf:defsystem ...)` blocks and their `:depends-on (...)` lists
//! without executing Lisp, using the shared [`crate::sexpr`] parser.

use crate::sexpr::Sexp;
use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `*.asd` files.
///
/// Since ASDF files use non-standard filenames (e.g. `my-system.asd`), this
/// parser is not registered in `parse_manifest()` by filename. Use
/// `parse_manifest_by_extension("asd", content)` or call `AsdParser` directly.
pub struct AsdParser;

impl ManifestParser for AsdParser {
    fn filename(&self) -> &'static str {
        "*.asd"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        for token in &Sexp::parse(content) {
            let Some(children) = token.as_list() else {
                continue;
            };
            let head = match children.first() {
                Some(Sexp::Atom(s)) => s.as_str(),
                _ => continue,
            };
            if head == "asdf:defsystem" || head == "defsystem" || head == "asdf/defsystem:defsystem"
            {
                parse_defsystem(children, &mut name, &mut version, &mut deps);
            }
        }

        Ok(ParsedManifest {
            ecosystem: "quicklisp",
            name,
            version,
            dependencies: deps,
        })
    }
}

fn parse_defsystem(
    children: &[Sexp],
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    // children[0] = "defsystem", children[1] = system-name, rest = keyword/value pairs.
    if name.is_none()
        && let Some(sys_name) = children.get(1).and_then(|t| t.as_text())
    {
        *name = Some(strip_hash_colon(sys_name));
    }

    let mut i = 2;
    while i < children.len() {
        if let Sexp::Atom(kw) = &children[i] {
            match kw.to_lowercase().as_str() {
                ":version" => {
                    if version.is_none()
                        && let Some(v) = children.get(i + 1).and_then(|t| t.as_text())
                    {
                        *version = Some(v.to_string());
                    }
                    i += 2;
                    continue;
                }
                ":depends-on" => {
                    if let Some(dep_list) = children.get(i + 1).and_then(|t| t.as_list()) {
                        parse_depends_on(dep_list, deps);
                    }
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        i += 1;
    }
}

fn parse_depends_on(dep_list: &[Sexp], deps: &mut Vec<DeclaredDep>) {
    for token in dep_list {
        match token {
            Sexp::Atom(s) => {
                let dep_name = strip_hash_colon(s);
                if !dep_name.is_empty() {
                    deps.push(DeclaredDep {
                        name: dep_name,
                        version_req: None,
                        kind: DepKind::Normal,
                    });
                }
            }
            Sexp::Str(s) => {
                if !s.is_empty() {
                    deps.push(DeclaredDep {
                        name: s.clone(),
                        version_req: None,
                        kind: DepKind::Normal,
                    });
                }
            }
            Sexp::List(sub) => {
                if let Some(dep) = parse_dep_form(sub) {
                    deps.push(dep);
                }
            }
        }
    }
}

/// Parse compound dependency forms:
/// - `(:version #:name "ver")` → Normal with version_req
/// - `(:feature :platform #:name)` → Optional
/// - `(:require ...)` → skip
fn parse_dep_form(sub: &[Sexp]) -> Option<DeclaredDep> {
    let head = match sub.first() {
        Some(Sexp::Atom(s)) => s.to_lowercase(),
        _ => return None,
    };

    match head.as_str() {
        ":version" => {
            // (:version #:name "ver")
            let dep_name = sub.get(1)?.as_text().map(strip_hash_colon)?;
            if dep_name.is_empty() {
                return None;
            }
            let version_req = sub.get(2).and_then(|t| t.as_text()).map(|s| s.to_string());
            Some(DeclaredDep {
                name: dep_name,
                version_req,
                kind: DepKind::Normal,
            })
        }
        ":feature" => {
            // (:feature :keyword #:dep-name)
            // The dep name is the last non-keyword symbol in the form.
            let dep_name = sub.iter().rev().find_map(|t| {
                if let Sexp::Atom(s) = t {
                    let stripped = strip_hash_colon(s);
                    if !stripped.is_empty() && !stripped.starts_with(':') {
                        return Some(stripped);
                    }
                }
                None
            })?;
            Some(DeclaredDep {
                name: dep_name,
                version_req: None,
                kind: DepKind::Optional,
            })
        }
        _ => None,
    }
}

fn strip_hash_colon(s: &str) -> String {
    s.trim_start_matches('#')
        .trim_start_matches(':')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    const SAMPLE: &str = r#"(asdf:defsystem #:my-system
  :name "my-system"
  :version "1.0.0"
  :description "My CL system"
  :depends-on (#:alexandria
               #:cl-ppcre
               (:version #:bordeaux-threads "0.8.0")
               (:feature :sbcl #:sb-posix))
  :in-order-to ((test-op (test-op #:my-system/tests))))

(asdf:defsystem #:my-system/tests
  :depends-on (#:my-system #:fiveam))
"#;

    #[test]
    fn test_parse_asd() {
        let m = AsdParser.parse(SAMPLE).unwrap();
        assert_eq!(m.ecosystem, "quicklisp");
        assert_eq!(m.name.as_deref(), Some("my-system"));
        assert_eq!(m.version.as_deref(), Some("1.0.0"));

        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"alexandria"), "{names:?}");
        assert!(names.contains(&"cl-ppcre"), "{names:?}");
        assert!(names.contains(&"bordeaux-threads"), "{names:?}");
        assert!(names.contains(&"sb-posix"), "{names:?}");

        let bt = m
            .dependencies
            .iter()
            .find(|d| d.name == "bordeaux-threads")
            .unwrap();
        assert_eq!(bt.version_req.as_deref(), Some("0.8.0"));
        assert_eq!(bt.kind, DepKind::Normal);

        let sbposix = m
            .dependencies
            .iter()
            .find(|d| d.name == "sb-posix")
            .unwrap();
        assert_eq!(sbposix.kind, DepKind::Optional);
    }

    #[test]
    fn test_multiple_systems_deps_merged() {
        // Only the first system's name/version are used; both systems' deps are
        // collected when iterating over top-level forms.
        let m = AsdParser.parse(SAMPLE).unwrap();
        let names: Vec<&str> = m.dependencies.iter().map(|d| d.name.as_str()).collect();
        // my-system/tests also depends on my-system and fiveam.
        assert!(names.contains(&"fiveam"), "{names:?}");
    }
}
