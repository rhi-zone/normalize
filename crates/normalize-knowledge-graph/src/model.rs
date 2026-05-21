//! Core data types for the knowledge graph.
//!
//! A **Unit** is an addressable document stored as `<id>.md` with YAML frontmatter.
//! Outgoing edges are stored in each unit's `links` frontmatter field.
//! An **Edge** is a projected graph edge (materialized from a unit's links for query results).

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
// Link (outgoing edge stored in unit frontmatter)
// ---------------------------------------------------------------------------

/// An outgoing edge stored in a unit's `links` frontmatter field.
///
/// Direction is implicit: source is the unit containing this link, target is `to`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Link {
    pub kind: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "is_null_or_empty")]
    pub metadata: serde_json::Value,
}

fn is_null_or_empty(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => true,
        serde_json::Value::Object(m) => m.is_empty(),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Unit
// ---------------------------------------------------------------------------

/// A knowledge graph unit: frontmatter metadata + markdown body.
///
/// Stored as `<id>.md` in `.normalize/kg/`.
/// Outgoing edges are stored in the `links` frontmatter field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    /// Stable identifier. Grammar: `[a-z0-9][a-z0-9-]*`.
    pub id: String,
    /// Arbitrary YAML frontmatter (as JSON Value for uniform handling).
    /// Does not include the `links` field (links are stored separately).
    pub metadata: serde_json::Value,
    /// Outgoing edges from this unit to other units.
    #[serde(default)]
    pub links: Vec<Link>,
    /// Markdown body (everything after the frontmatter block).
    pub body: String,
}

// ---------------------------------------------------------------------------
// Edge (projected for query results)
// ---------------------------------------------------------------------------

/// The projected state of a graph edge (materialized from unit frontmatter for queries).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Edge {
    /// Create an Edge from a unit's link.
    pub fn from_link(from: &str, link: &Link) -> Self {
        Self {
            from: from.to_string(),
            to: link.to.clone(),
            kind: link.kind.clone(),
            metadata: link.metadata.clone(),
        }
    }
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

    #[test]
    fn test_unit_roundtrip_with_links() {
        // Simulated: unit with links round-trips through serde_yaml
        let unit = Unit {
            id: "foo".to_string(),
            metadata: serde_json::json!({"tag": "wiki-page"}),
            links: vec![
                Link {
                    kind: "references".to_string(),
                    to: "bar".to_string(),
                    metadata: serde_json::Value::Null,
                },
                Link {
                    kind: "derived-from".to_string(),
                    to: "baz".to_string(),
                    metadata: serde_json::json!({"note": "see baz for background"}),
                },
            ],
            body: "Hello world\n".to_string(),
        };
        // Verify links are serializable/deserializable with serde_json
        let json = serde_json::to_string(&unit).unwrap();
        let parsed: Unit = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.links.len(), 2);
        assert_eq!(parsed.links[0].kind, "references");
        assert_eq!(parsed.links[0].to, "bar");
        assert_eq!(parsed.links[1].metadata["note"], "see baz for background");
    }

    #[test]
    fn test_unit_roundtrip_empty_links() {
        let unit = Unit {
            id: "foo".to_string(),
            metadata: serde_json::json!({}),
            links: vec![],
            body: "body\n".to_string(),
        };
        let json = serde_json::to_string(&unit).unwrap();
        let parsed: Unit = serde_json::from_str(&json).unwrap();
        assert!(parsed.links.is_empty());
    }

    #[test]
    fn test_edge_from_link() {
        let link = Link {
            kind: "references".to_string(),
            to: "target".to_string(),
            metadata: serde_json::json!({"weight": 1}),
        };
        let edge = Edge::from_link("source", &link);
        assert_eq!(edge.from, "source");
        assert_eq!(edge.to, "target");
        assert_eq!(edge.kind, "references");
        assert_eq!(edge.metadata["weight"], 1);
    }
}
