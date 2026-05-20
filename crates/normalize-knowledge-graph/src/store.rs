//! Filesystem read/write for the knowledge graph.
//!
//! Units live at `<kg_dir>/<id>.md` with YAML frontmatter.
//! Edges live at `<kg_dir>/edges.jsonl` (append-only, one JSON object per line).

use crate::model::{Edge, EdgeOp, EdgeOpKind, Unit, validate_id};
use std::collections::HashMap;
use std::io::{BufRead, Write};
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
    let (metadata, body) = parse_unit_file(&contents)?;
    Ok(Some(Unit {
        id: id.to_string(),
        metadata,
        body,
    }))
}

/// Write a unit to disk (creates or overwrites).
pub fn write_unit(kg_dir: &Path, unit: &Unit) -> Result<(), String> {
    let path = unit_path(kg_dir, &unit.id);
    let contents = render_unit_file(&unit.metadata, &unit.body);
    std::fs::write(&path, contents).map_err(|e| format!("Failed to write {:?}: {}", path, e))
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
            // Skip non-ID files (e.g. edges.jsonl won't pass, but let's also skip
            // anything that doesn't pass the ID grammar)
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
// Frontmatter parsing / rendering
// ---------------------------------------------------------------------------

/// Parse a unit file into (metadata, body).
///
/// Supports the standard YAML frontmatter format:
/// ```text
/// ---
/// key: value
/// ---
/// body text
/// ```
fn parse_unit_file(contents: &str) -> Result<(serde_json::Value, String), String> {
    if !contents.starts_with("---") {
        // No frontmatter — treat whole file as body.
        return Ok((
            serde_json::Value::Object(Default::default()),
            contents.to_string(),
        ));
    }

    // Find the closing `---`
    let after_first = &contents[3..];
    let Some(close_idx) = after_first.find("\n---") else {
        // Malformed frontmatter — treat whole file as body.
        return Ok((
            serde_json::Value::Object(Default::default()),
            contents.to_string(),
        ));
    };

    let yaml_str = &after_first[..close_idx];
    // Everything after `\n---` (plus the newline that may follow)
    let rest = &after_first[close_idx + 4..]; // skip "\n---"
    let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();

    let metadata: serde_yaml::Value =
        serde_yaml::from_str(yaml_str).unwrap_or(serde_yaml::Value::Mapping(Default::default()));
    let metadata_json = yaml_to_json(metadata);

    Ok((metadata_json, body))
}

/// Render a unit to a file string with YAML frontmatter.
fn render_unit_file(metadata: &serde_json::Value, body: &str) -> String {
    // Convert JSON back to YAML for storage
    let yaml_str = json_to_yaml_string(metadata);
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

/// Convert serde_json::Value to a YAML string (for writing frontmatter).
fn json_to_yaml_string(v: &serde_json::Value) -> String {
    // Convert JSON -> serde_yaml::Value -> YAML string
    let yaml_val: serde_yaml::Value = json_to_yaml_value(v);
    serde_yaml::to_string(&yaml_val).unwrap_or_default()
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
// Edge log I/O
// ---------------------------------------------------------------------------

fn edges_path(kg_dir: &Path) -> PathBuf {
    kg_dir.join("edges.jsonl")
}

/// Append one edge operation to the log.
pub fn append_edge_op(kg_dir: &Path, op: &EdgeOp) -> Result<(), String> {
    let path = edges_path(kg_dir);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open edges.jsonl: {}", e))?;
    let line =
        serde_json::to_string(op).map_err(|e| format!("Failed to serialize edge op: {}", e))?;
    writeln!(file, "{}", line).map_err(|e| format!("Failed to write edge op: {}", e))
}

/// Project the current edge set from the log.
///
/// Reads `edges.jsonl` line by line and applies ops in order:
/// - `add`: makes the edge present (most recent metadata wins).
/// - `remove`: removes the edge if present (no-op if not present).
pub fn project_edges(kg_dir: &Path) -> Result<Vec<Edge>, String> {
    let path = edges_path(kg_dir);
    if !path.exists() {
        return Ok(vec![]);
    }

    // Use an ordered map to preserve insertion order while supporting removal.
    // Key: (from, to, kind).
    let mut present: HashMap<(String, String, String), Edge> = HashMap::new();
    // Track insertion order separately.
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
        let op: EdgeOp = serde_json::from_str(line)
            .map_err(|e| format!("Failed to parse edge op at line {}: {}", line_no, e))?;

        let key = (op.from.clone(), op.to.clone(), op.kind.clone());
        match op.op {
            EdgeOpKind::Add => {
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
            EdgeOpKind::Remove => {
                present.remove(&key);
                // keep key in `order` but it won't be in `present` so it gets skipped below
            }
        }
    }

    // Return edges in insertion order (first-seen order).
    Ok(order
        .into_iter()
        .filter_map(|k| present.remove(&k))
        .collect())
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
        let rendered = render_unit_file(&metadata, body);
        let (parsed_meta, parsed_body) = parse_unit_file(&rendered).unwrap();
        assert_eq!(parsed_body, body);
        assert_eq!(parsed_meta["tag"], "wiki-page");
        assert_eq!(parsed_meta["status"], "draft");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let contents = "Just body text\n";
        let (meta, body) = parse_unit_file(contents).unwrap();
        assert_eq!(meta, serde_json::Value::Object(Default::default()));
        assert_eq!(body, contents);
    }

    #[test]
    fn test_edge_projection() {
        let dir = tempfile::tempdir().unwrap();
        let kg = dir.path();

        let t = "2026-01-01T00:00:00Z";

        // Add two edges
        let op1 = EdgeOp {
            op: EdgeOpKind::Add,
            from: "a".to_string(),
            to: "b".to_string(),
            kind: "references".to_string(),
            metadata: serde_json::Value::Null,
            created: t.to_string(),
        };
        let op2 = EdgeOp {
            op: EdgeOpKind::Add,
            from: "a".to_string(),
            to: "c".to_string(),
            kind: "references".to_string(),
            metadata: serde_json::Value::Null,
            created: t.to_string(),
        };
        let op3 = EdgeOp {
            op: EdgeOpKind::Remove,
            from: "a".to_string(),
            to: "b".to_string(),
            kind: "references".to_string(),
            metadata: serde_json::Value::Null,
            created: t.to_string(),
        };

        append_edge_op(kg, &op1).unwrap();
        append_edge_op(kg, &op2).unwrap();
        append_edge_op(kg, &op3).unwrap();

        let edges = project_edges(kg).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, "a");
        assert_eq!(edges[0].to, "c");
    }

    #[test]
    fn test_duplicate_add_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let kg = dir.path();
        let t = "2026-01-01T00:00:00Z";

        for i in 0..3 {
            let op = EdgeOp {
                op: EdgeOpKind::Add,
                from: "a".to_string(),
                to: "b".to_string(),
                kind: "ref".to_string(),
                metadata: serde_json::json!({"n": i}),
                created: t.to_string(),
            };
            append_edge_op(kg, &op).unwrap();
        }

        let edges = project_edges(kg).unwrap();
        assert_eq!(edges.len(), 1);
        // Most recent metadata wins
        assert_eq!(edges[0].metadata["n"], 2);
    }
}
