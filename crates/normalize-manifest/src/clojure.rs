//! Parsers for Clojure manifest files.
//!
//! - `LeinParser`: `project.clj` (Leiningen) — extracts `defproject` name/version
//!   and `[group/artifact "version"]` deps from `:dependencies`. Dev deps from
//!   `:profiles {:dev {:dependencies [...]}}`.
//! - `EclojureParser`: `deps.edn` (Clojure CLI) — extracts `{dep/name {:mvn/version "x"}}`
//!   pairs from `:deps`. Alias deps (`:dev`, `:test`) → `DepKind::Dev`.

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

// ============================================================================
// Leiningen — project.clj
// ============================================================================

/// Parser for `project.clj` files (Leiningen/Clojars).
pub struct LeinParser;

impl ManifestParser for LeinParser {
    fn filename(&self) -> &'static str {
        "project.clj"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // Extract name and version from: (defproject myapp "0.1.0-SNAPSHOT"
        if let Some(line) = content
            .lines()
            .find(|l| l.trim_start().starts_with("(defproject"))
        {
            let header = parse_defproject_header(line);
            name = header.name;
            version = header.version;
        }

        // Extract all dep vectors from the whole content.
        // We parse the full content tracking whether we're in a :dev profile.
        extract_lein_deps(content, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "clojars",
            name,
            version,
            dependencies: deps,
        })
    }
}

struct ProjectHeader {
    name: Option<String>,
    version: Option<String>,
}

struct DepVectors {
    deps: Vec<DeclaredDep>,
    bytes_consumed: usize,
}

struct BracedContent {
    content: String,
    chars_consumed: usize,
}

/// Parse `(defproject myapp "0.1.0-SNAPSHOT" ...` → name + version.
fn parse_defproject_header(line: &str) -> ProjectHeader {
    // Tokens after `defproject`
    let after = match line.find("defproject") {
        Some(i) => &line[i + "defproject".len()..],
        None => {
            return ProjectHeader {
                name: None,
                version: None,
            };
        }
    };

    let mut tokens = after.split_whitespace();
    let name = tokens
        .next()
        .map(|t| t.trim_matches(['(', ')']).to_string());
    let version = tokens
        .next()
        .map(|t| t.trim_matches(['"', '\'', '(', ')']).to_string());

    ProjectHeader { name, version }
}

/// Extract Leiningen dependency vectors from content.
///
/// Heuristically scans for `[group/artifact "version"]` patterns.
/// We track `:profiles {:dev ...}` by scanning for the `:profiles` keyword
/// and then checking if we're inside a `:dev` or `:test` block.
fn extract_lein_deps(content: &str, deps: &mut Vec<DeclaredDep>) {
    // State machine: find `:dependencies [` blocks and their context.
    // We do a text scan rather than full EDN parsing.
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for `:dependencies`
        if let Some(pos) = find_keyword(content, i, ":dependencies") {
            let after_kw = pos + ":dependencies".len();
            // Skip whitespace, then expect `[`
            let bracket_pos = content[after_kw..].find('[').map(|p| after_kw + p);

            if let Some(bp) = bracket_pos {
                // Determine if this `:dependencies` is inside a dev/test profile
                let is_dev = is_inside_dev_profile(content, pos);
                let kind = if is_dev {
                    DepKind::Dev
                } else {
                    DepKind::Normal
                };

                // Extract all `[name "version"]` entries inside this bracket
                let extracted = extract_dep_vectors(&content[bp..], kind);
                deps.extend(extracted.deps);
                i = bp + extracted.bytes_consumed;
                continue;
            } else {
                i = after_kw;
                continue;
            }
        } else {
            break;
        }
    }
}

/// Find `keyword` starting at or after `from` in `s`. Returns byte position.
fn find_keyword(s: &str, from: usize, keyword: &str) -> Option<usize> {
    s[from..].find(keyword).map(|p| from + p)
}

/// Returns true if the position `pos` in `content` appears to be inside a
/// `:dev` or `:test` profile block. Heuristic: look backwards for `:dev` or
/// `:test` before the `:dependencies` token, within a `:profiles` context.
fn is_inside_dev_profile(content: &str, dep_pos: usize) -> bool {
    let snippet = &content[..dep_pos];
    // Must have :profiles somewhere before this point
    if !snippet.contains(":profiles") {
        return false;
    }
    // Find the last `:dev` or `:test` before this position
    let last_dev = snippet.rfind(":dev");
    let last_test = snippet.rfind(":test");
    let last_profile_marker = match (last_dev, last_test) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    // Also find the last top-level :dependencies (position 0 context)
    // If the last profile marker is closer to dep_pos than the :profiles keyword, we're in it.
    last_profile_marker.is_some()
}

/// Extract `[name "version"]` dep vectors starting from an outer `[`.
/// Returns deps and bytes consumed from start of outer bracket.
fn extract_dep_vectors(s: &str, kind: DepKind) -> DepVectors {
    let mut deps = Vec::new();
    let mut depth = 0i32;
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    let total = chars.len();

    while i < total {
        match chars[i] {
            '[' => {
                depth += 1;
                if depth == 2 {
                    // Start of a dep vector — collect until matching `]`
                    let start = i;
                    let mut j = i + 1;
                    let mut inner_depth = 1i32;
                    while j < total {
                        match chars[j] {
                            '[' => inner_depth += 1,
                            ']' => {
                                inner_depth -= 1;
                                if inner_depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    let vec_str: String = chars[start..=j].iter().collect();
                    if let Some(dep) = parse_lein_dep_vector(&vec_str, kind) {
                        deps.push(dep);
                    }
                    // We consumed the `[...]` pair, so balance depth back to 1
                    depth -= 1;
                    i = j + 1;
                    continue;
                }
            }
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return DepVectors {
                        deps,
                        bytes_consumed: char_byte_offset(s, i + 1),
                    };
                }
            }
            _ => {}
        }
        i += 1;
    }

    DepVectors {
        deps,
        bytes_consumed: s.len(),
    }
}

fn char_byte_offset(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Parse `[group/artifact "1.0.0"]` → DeclaredDep.
fn parse_lein_dep_vector(s: &str, kind: DepKind) -> Option<DeclaredDep> {
    // Strip outer brackets
    let inner = s
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    if inner.is_empty() {
        return None;
    }

    // First token is the artifact name
    let mut tokens = inner.split_whitespace();
    let name_token = tokens.next()?.trim_matches(['"', '\'']);
    if name_token.is_empty() {
        return None;
    }

    // Second token (if present) is the version string
    let version_req = tokens
        .next()
        .map(|t| t.trim_matches(['"', '\'', ',']))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string());

    Some(DeclaredDep {
        name: name_token.to_string(),
        version_req,
        kind,
    })
}

// ============================================================================
// Clojure CLI — deps.edn
// ============================================================================

/// Parser for `deps.edn` files (Clojure CLI / clojars).
pub struct EclojureParser;

impl ManifestParser for EclojureParser {
    fn filename(&self) -> &'static str {
        "deps.edn"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // Extract top-level :deps block
        extract_edn_deps(content, DepKind::Normal, &mut deps);

        // Extract :aliases blocks for :dev and :test
        extract_edn_alias_deps(content, &mut deps);

        Ok(ParsedManifest {
            ecosystem: "clojars",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

/// Extract `{dep/name {:mvn/version "x"}}` pairs from a `:deps` map in `content`.
fn extract_edn_deps(content: &str, kind: DepKind, deps: &mut Vec<DeclaredDep>) {
    let Some(kw_pos) = content.find(":deps") else {
        return;
    };
    let after = &content[kw_pos + ":deps".len()..];
    let Some(brace_pos) = after.find('{') else {
        return;
    };
    let map_str = &after[brace_pos..];
    parse_edn_dep_map(map_str, kind, deps);
}

/// Scan `:aliases` block and extract `:dev`/`:test` extra-deps as Dev.
fn extract_edn_alias_deps(content: &str, deps: &mut Vec<DeclaredDep>) {
    let Some(aliases_pos) = content.find(":aliases") else {
        return;
    };
    let after_aliases = &content[aliases_pos + ":aliases".len()..];
    let Some(outer_brace) = after_aliases.find('{') else {
        return;
    };
    // Extract the outer aliases map
    let aliases_map = &after_aliases[outer_brace..];

    // Find each :dev and :test block inside
    for marker in &[":dev", ":test"] {
        let mut search_start = 0;
        while let Some(rel) = aliases_map[search_start..].find(marker) {
            let abs = search_start + rel;
            // Make sure it's a keyword (preceded by whitespace or `{`)
            let before = &aliases_map[..abs];
            let is_keyword = before
                .chars()
                .last()
                .map(|c| c.is_whitespace() || c == '{')
                .unwrap_or(true);
            if !is_keyword {
                search_start = abs + marker.len();
                continue;
            }

            let after_marker = &aliases_map[abs + marker.len()..];
            // Look for :extra-deps inside this alias block
            if let Some(ed_pos) = after_marker.find(":extra-deps") {
                let after_ed = &after_marker[ed_pos + ":extra-deps".len()..];
                if let Some(b) = after_ed.find('{') {
                    let map_str = &after_ed[b..];
                    parse_edn_dep_map(map_str, DepKind::Dev, deps);
                }
            }
            search_start = abs + marker.len();
        }
    }
}

/// Parse an EDN map of the form `{dep/name {:mvn/version "x"} ...}` into deps.
fn parse_edn_dep_map(s: &str, kind: DepKind, deps: &mut Vec<DeclaredDep>) {
    // We scan character by character tracking brace depth.
    // At depth 1 (inside the outer map), we collect symbol tokens as dep names
    // and then look for {:mvn/version "x"} values.
    let chars: Vec<char> = s.chars().collect();
    let total = chars.len();
    let mut i = 0;

    // Skip to opening brace
    if chars.is_empty() || chars[0] != '{' {
        return;
    }
    i += 1; // past '{'

    while i < total {
        // Skip whitespace and commas
        while i < total && (chars[i].is_whitespace() || chars[i] == ',') {
            i += 1;
        }
        if i >= total || chars[i] == '}' {
            break;
        }

        // Read the dep name symbol (e.g., `org.clojure/clojure`)
        if chars[i] == ';' {
            // EDN line comment — skip to end of line
            while i < total && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        let name_start = i;
        while i < total
            && !chars[i].is_whitespace()
            && chars[i] != '{'
            && chars[i] != '}'
            && chars[i] != ','
        {
            i += 1;
        }
        let dep_name: String = chars[name_start..i].iter().collect();
        let dep_name = dep_name.trim_matches(['"', '\'', ':']);

        if dep_name.is_empty() {
            i += 1;
            continue;
        }

        // Skip whitespace
        while i < total && chars[i].is_whitespace() {
            i += 1;
        }

        if i >= total {
            break;
        }

        // Read the value — should be a map `{...}`
        if chars[i] == '{' {
            let braced = extract_braced(&chars[i..]);
            let version_req = extract_mvn_version(&braced.content);
            deps.push(DeclaredDep {
                name: dep_name.to_string(),
                version_req,
                kind,
            });
            i += braced.chars_consumed;
        } else {
            // Unexpected token — skip
            i += 1;
        }
    }
}

/// Extract a `{...}` block from a char slice (including nested braces).
fn extract_braced(chars: &[char]) -> BracedContent {
    let mut depth = 0i32;
    let mut result = String::new();
    for (idx, &ch) in chars.iter().enumerate() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    result.push(ch);
                    return BracedContent {
                        content: result,
                        chars_consumed: idx + 1,
                    };
                }
            }
            _ => {}
        }
        result.push(ch);
    }
    BracedContent {
        content: result,
        chars_consumed: chars.len(),
    }
}

/// Extract `:mvn/version "x"` from a string like `{:mvn/version "1.11.1"}`.
fn extract_mvn_version(s: &str) -> Option<String> {
    let kw = ":mvn/version";
    let pos = s.find(kw)?;
    let after = s[pos + kw.len()..].trim_start();
    // Next quoted string
    let quote_start = after.find('"')?;
    let inner = &after[quote_start + 1..];
    let quote_end = inner.find('"')?;
    Some(inner[..quote_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_project_clj() {
        let content = r#"(defproject myapp "0.1.0-SNAPSHOT"
  :description "My application"
  :url "http://example.com"
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [ring/ring-core "1.9.6"]
                 [compojure "1.7.0"]]
  :profiles {:dev {:dependencies [[midje "1.10.9"]
                                  [ring/ring-mock "0.4.0"]]}})
"#;
        let m = LeinParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "clojars");
        assert_eq!(m.name.as_deref(), Some("myapp"));
        assert_eq!(m.version.as_deref(), Some("0.1.0-SNAPSHOT"));

        let clojure = m
            .dependencies
            .iter()
            .find(|d| d.name == "org.clojure/clojure")
            .unwrap();
        assert_eq!(clojure.version_req.as_deref(), Some("1.11.1"));
        assert_eq!(clojure.kind, DepKind::Normal);

        let ring = m
            .dependencies
            .iter()
            .find(|d| d.name == "ring/ring-core")
            .unwrap();
        assert_eq!(ring.version_req.as_deref(), Some("1.9.6"));

        let midje = m.dependencies.iter().find(|d| d.name == "midje").unwrap();
        assert_eq!(midje.kind, DepKind::Dev);
        assert_eq!(midje.version_req.as_deref(), Some("1.10.9"));

        let mock = m
            .dependencies
            .iter()
            .find(|d| d.name == "ring/ring-mock")
            .unwrap();
        assert_eq!(mock.kind, DepKind::Dev);
    }

    #[test]
    fn test_parse_deps_edn() {
        let content = r#"{:deps {org.clojure/clojure {:mvn/version "1.11.1"}
        ring/ring-core {:mvn/version "1.9.6"}
        io.github.user/mylib {:git/url "https://github.com/user/mylib"
                              :git/sha "abc123def456"}}
 :aliases {:dev {:extra-deps {cider/cider-nrepl {:mvn/version "0.45.0"}
                               nrepl/nrepl {:mvn/version "1.0.0"}}}
           :test {:extra-deps {lambdaisland/kaocha {:mvn/version "1.87.1342"}}}}}
"#;
        let m = EclojureParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "clojars");

        let clojure = m
            .dependencies
            .iter()
            .find(|d| d.name == "org.clojure/clojure")
            .unwrap();
        assert_eq!(clojure.version_req.as_deref(), Some("1.11.1"));
        assert_eq!(clojure.kind, DepKind::Normal);

        let mylib = m
            .dependencies
            .iter()
            .find(|d| d.name == "io.github.user/mylib")
            .unwrap();
        // git dep has no :mvn/version
        assert!(mylib.version_req.is_none());

        let cider = m
            .dependencies
            .iter()
            .find(|d| d.name == "cider/cider-nrepl")
            .unwrap();
        assert_eq!(cider.kind, DepKind::Dev);
        assert_eq!(cider.version_req.as_deref(), Some("0.45.0"));

        let kaocha = m
            .dependencies
            .iter()
            .find(|d| d.name == "lambdaisland/kaocha")
            .unwrap();
        assert_eq!(kaocha.kind, DepKind::Dev);
    }
}
