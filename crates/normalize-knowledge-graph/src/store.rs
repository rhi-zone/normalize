//! Filesystem read/write for the knowledge graph.
//!
//! Units live at `<kg_dir>/<id>.md` with YAML frontmatter.
//! Outgoing edges are stored in each unit's `links` frontmatter field.
//! The old `edges.jsonl` log is no longer written; if present, it is migrated
//! on first access and renamed to `edges.jsonl.migrated-v0`.

use crate::model::{Edge, Link, Unit, validate_id};
use std::path::{Path, PathBuf};

/// Returns the knowledge-graph directory: `<normalize_dir>/kg/`.
pub fn kg_dir(normalize_dir: &Path) -> PathBuf {
    normalize_dir.join("kg")
}

/// Ensure the kg directory exists.
pub fn ensure_kg_dir(normalize_dir: &Path) -> Result<PathBuf, String> {
    let dir = kg_dir(normalize_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create kg directory {:?}: {}", dir, e))?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// Unit I/O
// ---------------------------------------------------------------------------

/// Read a unit from disk. Returns `None` if the file does not exist.
pub fn read_unit(kg_dir: &Path, id: &str) -> Result<Option<Unit>, String> {
    let path = unit_path(kg_dir, id);
    if !path.exists() {
        return Ok(None);
    }
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
    let (metadata, links, body) = parse_unit_file(&contents)?;
    Ok(Some(Unit {
        id: id.to_string(),
        metadata,
        links,
        body,
    }))
}

/// Write a unit to disk (creates or overwrites) atomically.
pub fn write_unit(kg_dir: &Path, unit: &Unit) -> Result<(), String> {
    let path = unit_path(kg_dir, &unit.id);
    let contents = render_unit_file(&unit.metadata, &unit.links, &unit.body);
    // Atomic write: write to temp file then rename
    let tmp_path = path.with_extension("md.tmp");
    std::fs::write(&tmp_path, &contents)
        .map_err(|e| format!("Failed to write temp {:?}: {}", tmp_path, e))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Failed to rename {:?} -> {:?}: {}", tmp_path, path, e))
}

/// Delete a unit file. Returns `true` if deleted, `false` if it didn't exist.
pub fn delete_unit(kg_dir: &Path, id: &str) -> Result<bool, String> {
    let path = unit_path(kg_dir, id);
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path).map_err(|e| format!("Failed to delete {:?}: {}", path, e))?;
    Ok(true)
}

/// List all unit IDs present in the kg directory.
pub fn list_units(kg_dir: &Path) -> Result<Vec<String>, String> {
    if !kg_dir.exists() {
        return Ok(vec![]);
    }
    let mut ids = vec![];
    let entries = std::fs::read_dir(kg_dir)
        .map_err(|e| format!("Failed to read kg dir {:?}: {}", kg_dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if let Some(id) = name_str.strip_suffix(".md") {
            // Skip non-ID files (anything that doesn't pass the ID grammar)
            if validate_id(id).is_ok() {
                ids.push(id.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

/// Read all units from the kg directory.
pub fn read_all_units(kg_dir: &Path) -> Result<Vec<Unit>, String> {
    let ids = list_units(kg_dir)?;
    let mut units = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(unit) = read_unit(kg_dir, &id)? {
            units.push(unit);
        }
    }
    Ok(units)
}

fn unit_path(kg_dir: &Path, id: &str) -> PathBuf {
    kg_dir.join(format!("{}.md", id))
}

// ---------------------------------------------------------------------------
// Edge operations (per-unit frontmatter)
// ---------------------------------------------------------------------------

/// Add a directed edge from `from` to `to` with `kind` and `metadata`.
///
/// Reads the source unit, deduplicates on `(kind, to)` (latest metadata wins),
/// and writes back atomically.
pub fn link(
    kg_dir: &Path,
    from: &str,
    to: &str,
    kind: &str,
    metadata: serde_json::Value,
) -> Result<(), String> {
    let mut unit =
        read_unit(kg_dir, from)?.ok_or_else(|| format!("Source unit '{}' not found.", from))?;

    // Deduplicate: remove existing entry for (kind, to) if present
    unit.links.retain(|l| !(l.kind == kind && l.to == to));
    // Append the new link
    unit.links.push(Link {
        kind: kind.to_string(),
        to: to.to_string(),
        metadata,
    });

    write_unit(kg_dir, &unit)
}

/// Remove a directed edge from `from` to `to` with `kind`.
///
/// Reads the source unit, filters out the matching link, and writes back atomically.
pub fn unlink(kg_dir: &Path, from: &str, to: &str, kind: &str) -> Result<(), String> {
    let mut unit =
        read_unit(kg_dir, from)?.ok_or_else(|| format!("Source unit '{}' not found.", from))?;

    unit.links.retain(|l| !(l.kind == kind && l.to == to));

    write_unit(kg_dir, &unit)
}

/// Project all edges from all units in the kg directory.
///
/// Walks all units, flattens each unit's `links` into `Edge` tuples with `from = unit.id`.
pub fn list_all_edges(kg_dir: &Path) -> Result<Vec<Edge>, String> {
    let units = read_all_units(kg_dir)?;
    let mut edges = Vec::new();
    for unit in &units {
        for link in &unit.links {
            edges.push(Edge::from_link(&unit.id, link));
        }
    }
    Ok(edges)
}

// ---------------------------------------------------------------------------
// Frontmatter parsing / rendering
// ---------------------------------------------------------------------------

/// Parse a unit file into (metadata, links, body).
///
/// Supports the standard YAML frontmatter format:
/// ```text
/// ---
/// key: value
/// links:
///   - kind: references
///     to: other-unit
/// ---
/// body text
/// ```
fn parse_unit_file(contents: &str) -> Result<(serde_json::Value, Vec<Link>, String), String> {
    if !contents.starts_with("---") {
        // No frontmatter — treat whole file as body.
        return Ok((
            serde_json::Value::Object(Default::default()),
            vec![],
            contents.to_string(),
        ));
    }

    // Find the closing `---`
    let after_first = &contents[3..];
    let Some(close_idx) = after_first.find("\n---") else {
        // Malformed frontmatter — treat whole file as body.
        return Ok((
            serde_json::Value::Object(Default::default()),
            vec![],
            contents.to_string(),
        ));
    };

    let yaml_str = &after_first[..close_idx];
    // Everything after `\n---` (plus the newline that may follow)
    let rest = &after_first[close_idx + 4..]; // skip "\n---"
    let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();

    let yaml_val: serde_yaml::Value =
        serde_yaml::from_str(yaml_str).unwrap_or(serde_yaml::Value::Mapping(Default::default()));

    // Extract `links` from the YAML mapping, convert the remainder to JSON metadata.
    let serde_yaml::Value::Mapping(mut map) = yaml_val else {
        return Ok((serde_json::Value::Object(Default::default()), vec![], body));
    };
    let links_key = serde_yaml::Value::String("links".to_string());
    let links_yaml = map.remove(&links_key);
    let metadata_json = yaml_to_json(serde_yaml::Value::Mapping(map));
    let links = match links_yaml {
        Some(links_val) => {
            let links_json = yaml_to_json(links_val);
            serde_json::from_value(links_json).unwrap_or_default()
        }
        None => vec![],
    };

    Ok((metadata_json, links, body))
}

/// Render a unit to a file string with YAML frontmatter.
///
/// If `links` is non-empty, they are rendered under the `links` key in frontmatter.
fn render_unit_file(metadata: &serde_json::Value, links: &[Link], body: &str) -> String {
    // Build the full frontmatter object: metadata fields + optional links
    let mut yaml_map = serde_yaml::Mapping::new();

    // Insert metadata fields
    if let serde_json::Value::Object(meta_map) = metadata {
        for (k, v) in meta_map {
            yaml_map.insert(serde_yaml::Value::String(k.clone()), json_to_yaml_value(v));
        }
    }

    // Append links if non-empty
    if !links.is_empty() {
        let links_json = serde_json::to_value(links).unwrap_or(serde_json::Value::Array(vec![]));
        yaml_map.insert(
            serde_yaml::Value::String("links".to_string()),
            json_to_yaml_value(&links_json),
        );
    }

    let yaml_val = serde_yaml::Value::Mapping(yaml_map);
    let yaml_str = serde_yaml::to_string(&yaml_val).unwrap_or_default();
    format!("---\n{}---\n{}", yaml_str, body)
}

/// Convert serde_yaml::Value to serde_json::Value.
fn yaml_to_json(v: serde_yaml::Value) -> serde_json::Value {
    match v {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s),
        serde_yaml::Value::Sequence(arr) => {
            serde_json::Value::Array(arr.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                if let serde_yaml::Value::String(key) = k {
                    obj.insert(key, yaml_to_json(v));
                }
            }
            serde_json::Value::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

fn json_to_yaml_value(v: &serde_json::Value) -> serde_yaml::Value {
    match v {
        serde_json::Value::Null => serde_yaml::Value::Null,
        serde_json::Value::Bool(b) => serde_yaml::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_yaml::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::Null
            }
        }
        serde_json::Value::String(s) => serde_yaml::Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            serde_yaml::Value::Sequence(arr.iter().map(json_to_yaml_value).collect())
        }
        serde_json::Value::Object(map) => {
            let mut mapping = serde_yaml::Mapping::new();
            for (k, v) in map {
                mapping.insert(serde_yaml::Value::String(k.clone()), json_to_yaml_value(v));
            }
            serde_yaml::Value::Mapping(mapping)
        }
    }
}

// ---------------------------------------------------------------------------
// Migration from edges.jsonl
// ---------------------------------------------------------------------------

/// Migrate the legacy `edges.jsonl` log into per-unit frontmatter if present.
///
/// On first call (when `edges.jsonl` exists):
/// 1. Projects the current edge state from the log.
/// 2. For each present edge, reads the source unit, appends the link (idempotent).
/// 3. Renames `edges.jsonl` → `edges.jsonl.migrated-v0`.
/// 4. Logs one line to stderr.
///
/// Subsequent calls are no-ops (the file is gone).
pub fn migrate_jsonl_if_present(kg_dir: &Path) -> Result<(), String> {
    let edges_path = kg_dir.join("edges.jsonl");
    if !edges_path.exists() {
        return Ok(());
    }

    let edges = project_legacy_edges(kg_dir)?;
    let count = edges.len();

    for edge in edges {
        // Only migrate if source unit exists; skip dangling edges
        if read_unit(kg_dir, &edge.from)?.is_some() {
            link(kg_dir, &edge.from, &edge.to, &edge.kind, edge.metadata)?;
        }
    }

    // Rename the log
    let migrated_path = kg_dir.join("edges.jsonl.migrated-v0");
    std::fs::rename(&edges_path, &migrated_path)
        .map_err(|e| format!("Failed to rename edges.jsonl: {}", e))?;

    eprintln!(
        "kg: migrated {} edges from edges.jsonl to unit frontmatter (legacy log renamed to edges.jsonl.migrated-v0)",
        count
    );

    Ok(())
}

/// Project the legacy edge log into a flat list of current edges.
///
/// Reads `edges.jsonl` line by line, applying add/remove ops in order.
fn project_legacy_edges(kg_dir: &Path) -> Result<Vec<Edge>, String> {
    use std::collections::HashMap;
    use std::io::BufRead;

    let path = kg_dir.join("edges.jsonl");
    if !path.exists() {
        return Ok(vec![]);
    }

    let mut present: HashMap<(String, String, String), Edge> = HashMap::new();
    let mut order: Vec<(String, String, String)> = Vec::new();

    let file =
        std::fs::File::open(&path).map_err(|e| format!("Failed to open edges.jsonl: {}", e))?;
    let reader = std::io::BufReader::new(file);

    for (line_no, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("Failed to read line {}: {}", line_no, e))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let op: LegacyEdgeOp = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse edge op at line {}: {}", line_no, e))?;

        let key = (op.from.clone(), op.to.clone(), op.kind.clone());
        match op.op.as_str() {
            "add" => {
                if !present.contains_key(&key) {
                    order.push(key.clone());
                }
                present.insert(
                    key,
                    Edge {
                        from: op.from,
                        to: op.to,
                        kind: op.kind,
                        metadata: op.metadata,
                    },
                );
            }
            "remove" => {
                present.remove(&key);
            }
            _ => {
                // Unknown op — skip
            }
        }
    }

    Ok(order
        .into_iter()
        .filter_map(|k| present.remove(&k))
        .collect())
}

/// Minimal representation of a legacy edges.jsonl line.
#[derive(serde::Deserialize)]
struct LegacyEdgeOp {
    op: String,
    from: String,
    to: String,
    kind: String,
    #[serde(default)]
    metadata: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_render_roundtrip() {
        let metadata = serde_json::json!({"tag": "wiki-page", "status": "draft"});
        let body = "Hello, world!\n";
        let links = vec![Link {
            kind: "references".to_string(),
            to: "other-unit".to_string(),
            metadata: serde_json::Value::Null,
        }];
        let rendered = render_unit_file(&metadata, &links, body);
        let (parsed_meta, parsed_links, parsed_body) = parse_unit_file(&rendered).unwrap();
        assert_eq!(parsed_body, body);
        assert_eq!(parsed_meta["tag"], "wiki-page");
        assert_eq!(parsed_meta["status"], "draft");
        assert_eq!(parsed_links.len(), 1);
        assert_eq!(parsed_links[0].kind, "references");
        assert_eq!(parsed_links[0].to, "other-unit");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let contents = "Just body text\n";
        let (meta, links, body) = parse_unit_file(contents).unwrap();
        assert_eq!(meta, serde_json::Value::Object(Default::default()));
        assert!(links.is_empty());
        assert_eq!(body, contents);
    }

    #[test]
    fn test_parse_frontmatter_no_links() {
        let metadata = serde_json::json!({"tag": "note"});
        let rendered = render_unit_file(&metadata, &[], "body\n");
        let (parsed_meta, parsed_links, parsed_body) = parse_unit_file(&rendered).unwrap();
        assert_eq!(parsed_meta["tag"], "note");
        assert!(parsed_links.is_empty());
        assert_eq!(parsed_body, "body\n");
    }

    #[test]
    fn test_link_stores_in_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let kg = dir.path();

        // Create source and target units
        let src = Unit {
            id: "src".to_string(),
            metadata: serde_json::json!({}),
            links: vec![],
            body: "Source\n".to_string(),
        };
        write_unit(kg, &src).unwrap();
        let tgt = Unit {
            id: "tgt".to_string(),
            metadata: serde_json::json!({}),
            links: vec![],
            body: "Target\n".to_string(),
        };
        write_unit(kg, &tgt).unwrap();

        // Link
        link(kg, "src", "tgt", "references", serde_json::Value::Null).unwrap();

        // Read back — link should be in frontmatter
        let unit = read_unit(kg, "src").unwrap().unwrap();
        assert_eq!(unit.links.len(), 1);
        assert_eq!(unit.links[0].kind, "references");
        assert_eq!(unit.links[0].to, "tgt");
    }

    #[test]
    fn test_link_deduplicate_latest_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let kg = dir.path();

        let src = Unit {
            id: "src".to_string(),
            metadata: serde_json::json!({}),
            links: vec![],
            body: "".to_string(),
        };
        write_unit(kg, &src).unwrap();
        let tgt = Unit {
            id: "tgt".to_string(),
            metadata: serde_json::json!({}),
            links: vec![],
            body: "".to_string(),
        };
        write_unit(kg, &tgt).unwrap();

        // Link twice with different metadata — latest wins
        link(kg, "src", "tgt", "ref", serde_json::json!({"n": 1})).unwrap();
        link(kg, "src", "tgt", "ref", serde_json::json!({"n": 2})).unwrap();

        let unit = read_unit(kg, "src").unwrap().unwrap();
        assert_eq!(unit.links.len(), 1, "dedup: only one link for (kind, to)");
        assert_eq!(unit.links[0].metadata["n"], 2, "latest metadata wins");
    }

    #[test]
    fn test_unlink_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let kg = dir.path();

        for id in ["src", "tgt"] {
            let unit = Unit {
                id: id.to_string(),
                metadata: serde_json::json!({}),
                links: vec![],
                body: "".to_string(),
            };
            write_unit(kg, &unit).unwrap();
        }

        link(kg, "src", "tgt", "ref", serde_json::Value::Null).unwrap();
        unlink(kg, "src", "tgt", "ref").unwrap();

        let unit = read_unit(kg, "src").unwrap().unwrap();
        assert!(unit.links.is_empty(), "link should be gone after unlink");
    }

    #[test]
    fn test_list_all_edges_from_units() {
        let dir = tempfile::tempdir().unwrap();
        let kg = dir.path();

        for id in ["a", "b", "c"] {
            let unit = Unit {
                id: id.to_string(),
                metadata: serde_json::json!({}),
                links: vec![],
                body: "".to_string(),
            };
            write_unit(kg, &unit).unwrap();
        }

        link(kg, "a", "c", "ref", serde_json::Value::Null).unwrap();
        link(kg, "b", "c", "ref", serde_json::Value::Null).unwrap();

        let edges = list_all_edges(kg).unwrap();
        assert_eq!(edges.len(), 2);
        let froms: Vec<&str> = edges.iter().map(|e| e.from.as_str()).collect();
        assert!(froms.contains(&"a"));
        assert!(froms.contains(&"b"));
        let tos: Vec<&str> = edges.iter().map(|e| e.to.as_str()).collect();
        assert!(tos.iter().all(|t| *t == "c"));
    }

    #[test]
    fn test_migration_from_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let kg = dir.path();

        // Create units
        for id in ["a", "b", "c"] {
            let unit = Unit {
                id: id.to_string(),
                metadata: serde_json::json!({}),
                links: vec![],
                body: "".to_string(),
            };
            write_unit(kg, &unit).unwrap();
        }

        // Write a fake edges.jsonl with 3 adds and 1 remove
        let edges_path = kg.join("edges.jsonl");
        let lines = [
            r#"{"op":"add","from":"a","to":"b","kind":"ref","metadata":null,"created":"2026-01-01T00:00:00Z"}"#,
            r#"{"op":"add","from":"a","to":"c","kind":"ref","metadata":null,"created":"2026-01-01T00:00:00Z"}"#,
            r#"{"op":"add","from":"b","to":"c","kind":"uses","metadata":{"note":"test"},"created":"2026-01-01T00:00:00Z"}"#,
            r#"{"op":"remove","from":"a","to":"b","kind":"ref","metadata":null,"created":"2026-01-01T00:00:00Z"}"#,
        ];
        std::fs::write(&edges_path, lines.join("\n") + "\n").unwrap();

        // Migrate
        migrate_jsonl_if_present(kg).unwrap();

        // edges.jsonl should be renamed
        assert!(!edges_path.exists(), "edges.jsonl should be gone");
        assert!(
            kg.join("edges.jsonl.migrated-v0").exists(),
            "migrated file should exist"
        );

        // a should have link to c only (a->b was removed)
        let a = read_unit(kg, "a").unwrap().unwrap();
        assert_eq!(a.links.len(), 1);
        assert_eq!(a.links[0].to, "c");

        // b should have link to c with metadata
        let b = read_unit(kg, "b").unwrap().unwrap();
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.links[0].to, "c");
        assert_eq!(b.links[0].metadata["note"], "test");
    }
}
