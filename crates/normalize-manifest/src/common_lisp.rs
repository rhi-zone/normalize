//! Parser for `*.asd` files (Common Lisp/ASDF).
//!
//! ASDF system definition files use Common Lisp syntax. We heuristically
//! extract `(asdf:defsystem ...)` blocks and their `:depends-on (...)` lists
//! without executing Lisp.

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

        // Tokenise the whole file and scan for defsystem forms.
        let tokens: Vec<AsdToken> = tokenise_asd(content);
        let mut i = 0;
        while i < tokens.len() {
            if let AsdToken::List(children) = &tokens[i] {
                // Check for (asdf:defsystem ...) or (defsystem ...).
                let head = match children.first() {
                    Some(AsdToken::Symbol(s)) => s.as_str(),
                    _ => {
                        i += 1;
                        continue;
                    }
                };
                if head == "asdf:defsystem"
                    || head == "defsystem"
                    || head == "asdf/defsystem:defsystem"
                {
                    parse_defsystem(children, &mut name, &mut version, &mut deps);
                }
            }
            i += 1;
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
    children: &[AsdToken],
    name: &mut Option<String>,
    version: &mut Option<String>,
    deps: &mut Vec<DeclaredDep>,
) {
    // children[0] = "defsystem", children[1] = system-name, rest = keyword/value pairs.
    if name.is_none()
        && let Some(AsdToken::Symbol(sys_name)) = children.get(1)
    {
        *name = Some(strip_hash_colon(sys_name));
    }

    let mut i = 2;
    while i < children.len() {
        if let AsdToken::Symbol(kw) = &children[i] {
            match kw.to_lowercase().as_str() {
                ":version" => {
                    if let Some(AsdToken::Symbol(v)) = children.get(i + 1)
                        && version.is_none()
                    {
                        *version = Some(strip_quotes(v));
                    }
                    i += 2;
                    continue;
                }
                ":depends-on" => {
                    if let Some(AsdToken::List(dep_list)) = children.get(i + 1) {
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

fn parse_depends_on(dep_list: &[AsdToken], deps: &mut Vec<DeclaredDep>) {
    for token in dep_list {
        match token {
            AsdToken::Symbol(s) => {
                let dep_name = strip_hash_colon(s);
                if !dep_name.is_empty() {
                    deps.push(DeclaredDep {
                        name: dep_name,
                        version_req: None,
                        kind: DepKind::Normal,
                    });
                }
            }
            AsdToken::List(sub) => {
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
fn parse_dep_form(sub: &[AsdToken]) -> Option<DeclaredDep> {
    let head = match sub.first() {
        Some(AsdToken::Symbol(s)) => s.to_lowercase(),
        _ => return None,
    };

    match head.as_str() {
        ":version" => {
            // (:version #:name "ver")
            let dep_name = match sub.get(1) {
                Some(AsdToken::Symbol(s)) => strip_hash_colon(s),
                _ => return None,
            };
            let version_req = match sub.get(2) {
                Some(AsdToken::Symbol(s)) => Some(strip_quotes(s)),
                _ => None,
            };
            Some(DeclaredDep {
                name: dep_name,
                version_req,
                kind: DepKind::Normal,
            })
        }
        ":feature" => {
            // (:feature :keyword #:dep-name)
            // The dep name is the last symbol in the form.
            let dep_name = sub.iter().rev().find_map(|t| {
                if let AsdToken::Symbol(s) = t {
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
    let s = s.trim_start_matches('#').trim_start_matches(':');
    strip_quotes(s).to_string()
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
// Minimal Common Lisp tokeniser
// ============================================================================

#[derive(Debug, Clone)]
enum AsdToken {
    Symbol(String),
    List(Vec<AsdToken>),
}

fn tokenise_asd(input: &str) -> Vec<AsdToken> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < chars.len() {
        if let Some(tok) = read_asd_token(&chars, &mut pos) {
            out.push(tok);
        }
    }
    out
}

fn skip_asd_whitespace_comments(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() {
        match chars[*pos] {
            ' ' | '\t' | '\n' | '\r' => *pos += 1,
            ';' => {
                while *pos < chars.len() && chars[*pos] != '\n' {
                    *pos += 1;
                }
            }
            '#' if *pos + 1 < chars.len() && chars[*pos + 1] == '|' => {
                // Block comment #| ... |#
                *pos += 2;
                while *pos + 1 < chars.len() {
                    if chars[*pos] == '|' && chars[*pos + 1] == '#' {
                        *pos += 2;
                        break;
                    }
                    *pos += 1;
                }
            }
            _ => break,
        }
    }
}

fn read_asd_token(chars: &[char], pos: &mut usize) -> Option<AsdToken> {
    skip_asd_whitespace_comments(chars, pos);
    if *pos >= chars.len() {
        return None;
    }

    match chars[*pos] {
        '(' => {
            *pos += 1;
            let mut children = Vec::new();
            loop {
                skip_asd_whitespace_comments(chars, pos);
                if *pos >= chars.len() {
                    break;
                }
                if chars[*pos] == ')' {
                    *pos += 1;
                    break;
                }
                if let Some(tok) = read_asd_token(chars, pos) {
                    children.push(tok);
                }
            }
            Some(AsdToken::List(children))
        }
        ')' => {
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
            Some(AsdToken::Symbol(s))
        }
        '\'' => {
            // Quote reader macro — skip the ' and read the next token.
            *pos += 1;
            read_asd_token(chars, pos)
        }
        '`' | ',' => {
            *pos += 1;
            read_asd_token(chars, pos)
        }
        _ => {
            let mut s = String::new();
            while *pos < chars.len()
                && !matches!(
                    chars[*pos],
                    ' ' | '\t' | '\n' | '\r' | '(' | ')' | '"' | ';'
                )
            {
                s.push(chars[*pos]);
                *pos += 1;
            }
            if s.is_empty() {
                None
            } else {
                Some(AsdToken::Symbol(s))
            }
        }
    }
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
