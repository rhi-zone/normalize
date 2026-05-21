//! Predicate matching for knowledge graph query.
//!
//! Supports:
//! - `--match key=value` (dotted-path, string equality)
//! - `--edge-kind K` (filter edges by kind)
//! - `--connected-to ID` (filter units connected to a given unit)
//!
//! Edges are projected from per-unit frontmatter `links` fields via `store::list_all_edges`.

use crate::model::{Edge, Unit, dotted_lookup};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Match predicate
// ---------------------------------------------------------------------------

/// A single `key=value` match predicate.
#[derive(Debug, Clone)]
pub struct MatchPredicate {
    pub path: String,
    pub value: String,
}

impl MatchPredicate {
    /// Parse from `"key=value"` or `"dotted.key=value"` string.
    pub fn parse(s: &str) -> Result<Self, String> {
        let Some((path, value)) = s.split_once('=') else {
            return Err(format!(
                "Invalid --match predicate '{}': expected 'key=value'",
                s
            ));
        };
        Ok(Self {
            path: path.to_string(),
            value: value.to_string(),
        })
    }

    /// Returns `true` if the unit's metadata satisfies this predicate.
    pub fn matches_unit(&self, unit: &Unit) -> bool {
        let values = dotted_lookup(&unit.metadata, &self.path);
        values.contains(&self.value.as_str())
    }
}

// ---------------------------------------------------------------------------
// Unit filter
// ---------------------------------------------------------------------------

/// Filter units based on `--match` predicates and `--connected-to`.
///
/// All predicates are ANDed together.
pub fn filter_units<'a>(
    units: impl Iterator<Item = &'a Unit>,
    match_predicates: &[MatchPredicate],
    connected_to: Option<&str>,
    edges: &[Edge],
) -> Vec<&'a Unit> {
    units
        .filter(|unit| {
            // All --match predicates must hold
            for pred in match_predicates {
                if !pred.matches_unit(unit) {
                    return false;
                }
            }
            // --connected-to: unit must appear in edges adjacent to the given ID
            if let Some(target) = connected_to {
                let connected = edges.iter().any(|e| {
                    (e.from == unit.id && e.to == target) || (e.to == unit.id && e.from == target)
                });
                if !connected {
                    return false;
                }
            }
            true
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Edge filter
// ---------------------------------------------------------------------------

/// Filter edges by optional `--from`, `--to`, and `--kind`.
pub fn filter_edges<'a>(
    edges: impl Iterator<Item = &'a Edge>,
    from: Option<&str>,
    to: Option<&str>,
    kind: Option<&str>,
) -> Vec<&'a Edge> {
    edges
        .filter(|e| {
            if let Some(f) = from
                && e.from != f
            {
                return false;
            }
            if let Some(t) = to
                && e.to != t
            {
                return false;
            }
            if let Some(k) = kind
                && e.kind != k
            {
                return false;
            }
            true
        })
        .collect()
}

// ---------------------------------------------------------------------------
// BFS neighbors
// ---------------------------------------------------------------------------

/// Collect units reachable from `center_id` up to `depth` hops (BFS).
///
/// At depth 1 returns all units connected by any edge (in or out), de-duped.
/// Returns `(edge, unit)` pairs for each neighbor (may contain multiple entries
/// per unit when multiple edges connect the same pair).
pub fn bfs_neighbors<'a>(
    center_id: &str,
    depth: usize,
    edges: &'a [Edge],
    units: &'a [Unit],
    edge_kind: Option<&str>,
) -> Vec<(&'a Edge, &'a Unit)> {
    if depth == 0 {
        return vec![];
    }

    let unit_by_id: HashMap<&str, &Unit> = units.iter().map(|u| (u.id.as_str(), u)).collect();

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(center_id.to_string());

    let mut frontier: Vec<String> = vec![center_id.to_string()];
    let mut result: Vec<(&'a Edge, &'a Unit)> = Vec::new();

    for _ in 0..depth {
        let mut next_frontier: Vec<String> = Vec::new();
        for current in &frontier {
            for edge in edges {
                // Apply kind filter
                if let Some(k) = edge_kind
                    && edge.kind != k
                {
                    continue;
                }
                // Check if this edge is adjacent to `current`
                let neighbor_id = if edge.from == current.as_str() {
                    Some(edge.to.as_str())
                } else if edge.to == current.as_str() {
                    Some(edge.from.as_str())
                } else {
                    None
                };
                if let Some(nid) = neighbor_id
                    && let Some(unit) = unit_by_id.get(nid)
                {
                    result.push((edge, unit));
                    if !visited.contains(nid) {
                        visited.insert(nid.to_string());
                        next_frontier.push(nid.to_string());
                    }
                }
            }
        }
        frontier = next_frontier;
        if frontier.is_empty() {
            break;
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_unit(id: &str, meta: serde_json::Value, body: &str) -> Unit {
        Unit {
            id: id.to_string(),
            metadata: meta,
            links: vec![],
            body: body.to_string(),
        }
    }

    fn make_edge(from: &str, to: &str, kind: &str) -> Edge {
        Edge {
            from: from.to_string(),
            to: to.to_string(),
            kind: kind.to_string(),
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_match_predicate_simple() {
        let u = make_unit("a", json!({"tag": "wiki"}), "");
        let pred = MatchPredicate::parse("tag=wiki").unwrap();
        assert!(pred.matches_unit(&u));
        let pred2 = MatchPredicate::parse("tag=other").unwrap();
        assert!(!pred2.matches_unit(&u));
    }

    #[test]
    fn test_match_predicate_dotted() {
        let u = make_unit("a", json!({"anchors": {"symbol": "Frobnicator"}}), "");
        let pred = MatchPredicate::parse("anchors.symbol=Frobnicator").unwrap();
        assert!(pred.matches_unit(&u));
    }

    #[test]
    fn test_filter_edges() {
        let edges = [
            make_edge("a", "b", "ref"),
            make_edge("a", "c", "uses"),
            make_edge("b", "c", "ref"),
        ];
        let filtered = filter_edges(edges.iter(), Some("a"), None, None);
        assert_eq!(filtered.len(), 2);
        let filtered2 = filter_edges(edges.iter(), None, None, Some("ref"));
        assert_eq!(filtered2.len(), 2);
    }

    #[test]
    fn test_bfs_neighbors_depth1() {
        let edges = vec![make_edge("a", "b", "ref"), make_edge("a", "c", "ref")];
        let units = vec![
            make_unit("a", json!({}), ""),
            make_unit("b", json!({}), ""),
            make_unit("c", json!({}), ""),
        ];
        let neighbors = bfs_neighbors("a", 1, &edges, &units, None);
        assert_eq!(neighbors.len(), 2);
        let ids: Vec<&str> = neighbors.iter().map(|(_, u)| u.id.as_str()).collect();
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
    }

    #[test]
    fn test_bfs_neighbors_incoming() {
        let edges = vec![make_edge("b", "a", "ref")];
        let units = vec![make_unit("a", json!({}), ""), make_unit("b", json!({}), "")];
        let neighbors = bfs_neighbors("a", 1, &edges, &units, None);
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].1.id, "b");
    }
}
