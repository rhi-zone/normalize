//! Parser for `rebar.config` files (Erlang/rebar3).
//!
//! Heuristic Erlang term parsing:
//! - `{deps, [...]}` → `DepKind::Normal`
//! - `{profiles, [{dev, [{deps, [...]}]}, {test, [{deps, [...]}]}]}` → `DepKind::Dev`
//!
//! Dep formats recognized:
//! - `{name, "version"}` — hex package with version
//! - `{name, {git, URL, {tag, "version"}}}` — git dep with tag
//! - `name` — bare atom (no version)

use crate::{DeclaredDep, DepKind, ManifestError, ManifestParser, ParsedManifest};

/// Parser for `rebar.config` files (Erlang/rebar3).
pub struct RebarConfigParser;

impl ManifestParser for RebarConfigParser {
    fn filename(&self) -> &'static str {
        "rebar.config"
    }

    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError> {
        let mut deps: Vec<DeclaredDep> = Vec::new();

        // Find top-level {deps, [...]}
        if let Some(top_deps) = find_top_level_deps(content) {
            extract_rebar_deps(&top_deps, DepKind::Normal, &mut deps);
        }

        // Find {profiles, [...]} and extract dev/test deps
        if let Some(profiles) = find_profiles_block(content) {
            extract_profile_deps(&profiles, &mut deps);
        }

        Ok(ParsedManifest {
            ecosystem: "hex",
            name: None,
            version: None,
            dependencies: deps,
        })
    }
}

/// Find the content of the top-level `{deps, [...]}` tuple.
fn find_top_level_deps(content: &str) -> Option<String> {
    // Look for `{deps,` at low brace depth
    let mut depth = 0i32;
    let chars: Vec<char> = content.chars().collect();
    let total = chars.len();
    let mut i = 0;

    while i < total {
        match chars[i] {
            '{' => {
                depth += 1;
                // At depth 1 opening a new tuple
                if depth == 1 {
                    // Check if this is {deps, ...}
                    let rest: String = chars[i..].iter().take(10).collect();
                    let rest_trimmed = rest.trim_start_matches('{').trim_start();
                    if rest_trimmed.starts_with("deps") {
                        // Find the deps list
                        if let Some(bracket) = chars[i..].iter().position(|&c| c == '[') {
                            let list_start = i + bracket;
                            let list = extract_bracket_content(&chars[list_start..]);
                            return Some(list);
                        }
                    }
                }
            }
            '}' => depth -= 1,
            '%' => {
                // Erlang comment — skip to end of line
                while i < total && chars[i] != '\n' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Find the content inside the `{profiles, [...]}` tuple.
fn find_profiles_block(content: &str) -> Option<String> {
    let chars: Vec<char> = content.chars().collect();
    let total = chars.len();
    let mut i = 0;
    let mut depth = 0i32;

    while i < total {
        match chars[i] {
            '{' => {
                depth += 1;
                if depth == 1 {
                    let rest: String = chars[i..].iter().take(12).collect();
                    let inner = rest.trim_start_matches('{').trim_start();
                    if inner.starts_with("profiles") {
                        // Find the list [...]
                        if let Some(bracket) = chars[i..].iter().position(|&c| c == '[') {
                            let list_start = i + bracket;
                            let list = extract_bracket_content(&chars[list_start..]);
                            return Some(list);
                        }
                    }
                }
            }
            '}' => depth -= 1,
            '%' => {
                while i < total && chars[i] != '\n' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Extract profile deps from the content of `{profiles, [...]}`.
fn extract_profile_deps(profiles_content: &str, deps: &mut Vec<DeclaredDep>) {
    let chars: Vec<char> = profiles_content.chars().collect();
    let total = chars.len();
    let mut i = 0;

    while i < total {
        if chars[i] == '{' {
            // Read the profile name atom
            let tuple_start = i + 1;
            let atom_end = chars[tuple_start..]
                .iter()
                .position(|&c| c == ',' || c.is_whitespace())
                .map(|p| tuple_start + p)
                .unwrap_or(total);

            let profile_name: String = chars[tuple_start..atom_end].iter().collect();
            let profile_name = profile_name.trim();

            let is_dev = matches!(profile_name, "dev" | "test");

            // Inside this profile tuple, look for {deps, [...]}
            let tuple_content = extract_brace_content(&chars[i..]);
            if let Some(dep_list) = find_deps_in_string(&tuple_content) {
                let kind = if is_dev {
                    DepKind::Dev
                } else {
                    DepKind::Normal
                };
                extract_rebar_deps(&dep_list, kind, deps);
            }

            // Skip past this tuple
            let consumed = brace_len(&chars[i..]);
            i += consumed;
            continue;
        }
        i += 1;
    }
}

/// Find `{deps, [...]}` inside a string and return the bracket content.
fn find_deps_in_string(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let total = chars.len();
    let mut i = 0;

    while i < total {
        match chars[i] {
            '{' => {
                let rest: String = chars[i..].iter().take(8).collect();
                let inner = rest.trim_start_matches('{').trim_start();
                if inner.starts_with("deps")
                    && let Some(bracket) = chars[i..].iter().position(|&c| c == '[')
                {
                    let list_start = i + bracket;
                    return Some(extract_bracket_content(&chars[list_start..]));
                }
            }
            '}' => {}

            _ => {}
        }
        i += 1;
    }
    None
}

/// Extract deps from a `[...]` dep list string.
fn extract_rebar_deps(list_content: &str, kind: DepKind, out: &mut Vec<DeclaredDep>) {
    let chars: Vec<char> = list_content.chars().collect();
    let total = chars.len();
    let mut i = 0;

    // Skip opening `[`
    if !chars.is_empty() && chars[0] == '[' {
        i = 1;
    }

    while i < total {
        // Skip whitespace and commas
        while i < total && (chars[i].is_whitespace() || chars[i] == ',') {
            i += 1;
        }
        if i >= total || chars[i] == ']' {
            break;
        }

        if chars[i] == '%' {
            while i < total && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if chars[i] == '{' {
            // Tuple dep: {name, "version"} or {name, {git, ...}}
            let tuple = extract_brace_content(&chars[i..]);
            if let Some(dep) = parse_rebar_dep_tuple(&tuple, kind) {
                out.push(dep);
            }
            let consumed = brace_len(&chars[i..]);
            i += consumed;
        } else {
            // Bare atom dep
            let atom_start = i;
            while i < total
                && !chars[i].is_whitespace()
                && chars[i] != ','
                && chars[i] != ']'
                && chars[i] != '}'
            {
                i += 1;
            }
            let atom: String = chars[atom_start..i].iter().collect();
            let atom = atom.trim();
            if !atom.is_empty() {
                out.push(DeclaredDep {
                    name: atom.to_string(),
                    version_req: None,
                    kind,
                });
            }
        }
    }
}

/// Parse `{name, "version"}` or `{name, {git, URL, {tag, "version"}}}`.
fn parse_rebar_dep_tuple(s: &str, kind: DepKind) -> Option<DeclaredDep> {
    // Strip outer braces
    let inner = s
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();

    // First token is the dep name atom
    let comma_pos = inner.find(',')?;
    let name = inner[..comma_pos].trim().to_string();
    if name.is_empty() {
        return None;
    }

    let rest = inner[comma_pos + 1..].trim();

    // `"version"` — simple hex version
    if rest.starts_with('"') {
        let ver = rest.trim_matches('"').to_string();
        return Some(DeclaredDep {
            name,
            version_req: if ver.is_empty() { None } else { Some(ver) },
            kind,
        });
    }

    // `{git, URL, {tag, "version"}}` — extract tag version
    if rest.starts_with('{') {
        let tag_ver = extract_git_tag_version(rest);
        return Some(DeclaredDep {
            name,
            version_req: tag_ver,
            kind,
        });
    }

    // Fallback: dep name only
    Some(DeclaredDep {
        name,
        version_req: None,
        kind,
    })
}

/// Extract the tag version from `{git, URL, {tag, "3.9.2"}}`.
fn extract_git_tag_version(s: &str) -> Option<String> {
    let tag_pos = s.find("tag")?;
    let after_tag = &s[tag_pos + 3..].trim_start();
    // After "tag" there may be a comma then the version
    let after_comma = after_tag.trim_start_matches(',').trim_start();
    if let Some(inner) = after_comma.strip_prefix('"') {
        let end = inner.find('"')?;
        return Some(inner[..end].to_string());
    }
    None
}

/// Extract the full content (including outer `{...}`) as a string from a char slice.
fn extract_brace_content(chars: &[char]) -> String {
    let mut depth = 0i32;
    let mut result = String::new();
    for &ch in chars {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                result.push(ch);
                if depth == 0 {
                    return result;
                }
                continue;
            }
            _ => {}
        }
        result.push(ch);
    }
    result
}

/// Return number of chars consumed by one `{...}` block.
fn brace_len(chars: &[char]) -> usize {
    let mut depth = 0i32;
    for (i, &ch) in chars.iter().enumerate() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    chars.len()
}

/// Extract the full content (including outer `[...]`) as a string from a char slice.
fn extract_bracket_content(chars: &[char]) -> String {
    let mut depth = 0i32;
    let mut result = String::new();
    for &ch in chars {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                result.push(ch);
                if depth == 0 {
                    return result;
                }
                continue;
            }
            _ => {}
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManifestParser;

    #[test]
    fn test_parse_rebar_config() {
        let content = r#"{deps, [
    {cowboy, "2.10.0"},
    {jsx, "3.1.0"},
    {lager, {git, "https://github.com/erlang-lager/lager.git", {tag, "3.9.2"}}},
    jsx
]}.
{profiles, [
    {dev, [{deps, [
        {recon, "2.5.4"}
    ]}]},
    {test, [{deps, [
        {proper, "1.4.0"}
    ]}]}
]}.
"#;
        let m = RebarConfigParser.parse(content).unwrap();
        assert_eq!(m.ecosystem, "hex");

        let cowboy = m.dependencies.iter().find(|d| d.name == "cowboy").unwrap();
        assert_eq!(cowboy.kind, DepKind::Normal);
        assert_eq!(cowboy.version_req.as_deref(), Some("2.10.0"));

        let lager = m.dependencies.iter().find(|d| d.name == "lager").unwrap();
        assert_eq!(lager.kind, DepKind::Normal);
        assert_eq!(lager.version_req.as_deref(), Some("3.9.2"));

        // bare atom
        let jsx = m.dependencies.iter().find(|d| d.name == "jsx").unwrap();
        assert_eq!(jsx.kind, DepKind::Normal);

        let recon = m.dependencies.iter().find(|d| d.name == "recon").unwrap();
        assert_eq!(recon.kind, DepKind::Dev);

        let proper = m.dependencies.iter().find(|d| d.name == "proper").unwrap();
        assert_eq!(proper.kind, DepKind::Dev);
    }

    #[test]
    fn test_minimal_rebar() {
        let content = "{deps, [{cowboy, \"2.9.0\"}]}.\n";
        let m = RebarConfigParser.parse(content).unwrap();
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].name, "cowboy");
        assert_eq!(m.dependencies[0].version_req.as_deref(), Some("2.9.0"));
    }
}
