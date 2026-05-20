//! Core data types for the knowledge graph.
//!
//! A **Unit** is an addressable document stored as `<id>.md` with YAML frontmatter.
//! An **EdgeOp** is one line in the append-only `edges.jsonl` log.
//! An **Edge** is the current (projected) state of a graph edge.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ID validation
// ---------------------------------------------------------------------------

/// Validate that a knowledge graph ID matches `[a-z0-9][a-z0-9-]*`.
///
/// Returns `Ok(())` if valid, `Err` with a descriptive message otherwise.
pub fn validate_id(id: &str) -> Result<(), String> {
    let Some(first) = id.chars().next() else {
        return Err("ID must not be empty".to_string());
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(format!(
            "ID '{}' must start with [a-z0-9], got '{}'",
            id, first
        ));
    }
    for ch in id.chars() {
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' {
            return Err(format!(
                "ID '{}' contains invalid character '{}'. Only [a-z0-9-] allowed.",
                id, ch
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit
// ---------------------------------------------------------------------------

/// A knowledge graph unit: frontmatter metadata + markdown body.
///
/// Stored as `<id>.md` in `.normalize/kg/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    /// Stable identifier. Grammar: `[a-z0-9][a-z0-9-]*`.
    pub id: String,
    /// Arbitrary YAML frontmatter (as JSON Value for uniform handling).
    pub metadata: serde_json::Value,
    /// Markdown body (everything after the frontmatter block).
    pub body: String,
}

// ---------------------------------------------------------------------------
// Edge log
// ---------------------------------------------------------------------------

/// One line in the append-only `edges.jsonl` log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeOp {
    pub op: EdgeOpKind,
    pub from: String,
    pub to: String,
    pub kind: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub created: String,
}

/// The operation kind stored in `edges.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeOpKind {
    Add,
    Remove,
}

/// The current (projected) state of a graph edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Metadata helpers (used by query matching)
// ---------------------------------------------------------------------------

/// Look up a dotted-path key in a JSON value.
///
/// `anchors.symbol` resolves `value["anchors"]["symbol"]`. Returns all
/// matching string values: handles both a scalar and a list of objects.
pub fn dotted_lookup<'a>(value: &'a serde_json::Value, path: &str) -> Vec<&'a str> {
    let parts: Vec<&str> = path.splitn(2, '.').collect();
    let key = parts[0];
    let rest = parts.get(1).copied();

    match value {
        serde_json::Value::Object(map) => {
            let Some(child) = map.get(key) else {
                return vec![];
            };
            if let Some(rest) = rest {
                dotted_lookup(child, rest)
            } else {
                // leaf: collect string values (scalar or list of scalars)
                collect_strings(child)
            }
        }
        serde_json::Value::Array(arr) => {
            // Recurse into each element (supports list-of-objects for anchors)
            arr.iter()
                .flat_map(|item| dotted_lookup(item, path))
                .collect()
        }
        _ => vec![],
    }
}

fn collect_strings(v: &serde_json::Value) -> Vec<&str> {
    match v {
        serde_json::Value::String(s) => vec![s.as_str()],
        serde_json::Value::Array(arr) => arr.iter().flat_map(collect_strings).collect(),
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_id_valid() {
        assert!(validate_id("frobnicator-overview").is_ok());
        assert!(validate_id("foo").is_ok());
        assert!(validate_id("foo123").is_ok());
        assert!(validate_id("123abc").is_ok());
        assert!(validate_id("a").is_ok());
    }

    #[test]
    fn test_validate_id_invalid() {
        assert!(validate_id("").is_err());
        assert!(validate_id("-foo").is_err());
        assert!(validate_id("Foo").is_err());
        assert!(validate_id("foo_bar").is_err());
        assert!(validate_id("foo/bar").is_err());
        assert!(validate_id("FOO").is_err());
    }

    #[test]
    fn test_dotted_lookup_scalar() {
        let v: serde_json::Value = serde_json::json!({"tag": "wiki-page", "status": "draft"});
        assert_eq!(dotted_lookup(&v, "tag"), vec!["wiki-page"]);
        assert_eq!(dotted_lookup(&v, "missing"), Vec::<&str>::new());
    }

    #[test]
    fn test_dotted_lookup_nested() {
        let v: serde_json::Value = serde_json::json!({"anchors": {"symbol": "Frobnicator"}});
        assert_eq!(dotted_lookup(&v, "anchors.symbol"), vec!["Frobnicator"]);
    }

    #[test]
    fn test_dotted_lookup_list_of_objects() {
        let v: serde_json::Value = serde_json::json!({
            "anchors": [{"symbol": "Foo"}, {"symbol": "Bar"}]
        });
        let mut got = dotted_lookup(&v, "anchors.symbol");
        got.sort();
        assert_eq!(got, vec!["Bar", "Foo"]);
    }

    #[test]
    fn test_dotted_lookup_list_of_scalars() {
        let v: serde_json::Value = serde_json::json!({"tags": ["a", "b"]});
        let mut got = dotted_lookup(&v, "tags");
        got.sort();
        assert_eq!(got, vec!["a", "b"]);
    }
}
