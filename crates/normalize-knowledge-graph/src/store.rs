//! Filesystem read/write for the knowledge graph.
//!
//! Units live at `<kg_dir>/<id>.md` with YAML frontmatter.
//! Outgoing edges are stored in each unit's `links` frontmatter field.
//! The old `edges.jsonl` log is no longer written; if present, it is migrated
//! on first access and renamed to `edges.jsonl.migrated-v0`.

use crate::model::{Link, Unit, validate_id};
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
        if let Some(id) = name_str.strip_suffix(".md")
            && validate_id(id).is_ok()
        {
            ids.push(id.to_string());
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
// jq evaluation
// ---------------------------------------------------------------------------

#[cfg(feature = "cli")]
type D = jaq_core::data::JustLut<jaq_json::Val>;
#[cfg(feature = "cli")]
type CompiledFilter = jaq_core::compile::Filter<jaq_core::Native<D>>;

#[cfg(feature = "cli")]
fn jq_compile(expr: &str) -> Result<CompiledFilter, String> {
    use jaq_core::load::{Arena, File, Loader};

    let arena = Arena::default();
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let loader = Loader::new(defs);
    let modules = loader
        .load(
            &arena,
            File {
                code: expr,
                path: (),
            },
        )
        .map_err(|errs| {
            let msgs: Vec<String> = errs.into_iter().map(|(_, e)| format!("{e:?}")).collect();
            format!("jq parse error: {}", msgs.join("; "))
        })?;

    let funs = jaq_core::funs::<D>()
        .chain(jaq_std::funs::<D>())
        .chain(jaq_json::funs::<D>());
    jaq_core::Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|errs| {
            let msgs: Vec<String> = errs.into_iter().map(|(_, e)| format!("{e:?}")).collect();
            format!("jq compile error: {}", msgs.join("; "))
        })
}

#[cfg(feature = "cli")]
fn jq_val_to_json(val: &jaq_json::Val) -> Result<serde_json::Value, String> {
    serde_json::from_str(&format!("{val}"))
        .map_err(|e| format!("Failed to convert jq output to JSON: {}", e))
}

#[cfg(feature = "cli")]
fn jq_run_one(
    filter: &CompiledFilter,
    input: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use jaq_core::{Ctx, Vars};
    use jaq_json::Val;

    let val: Val = serde_json::from_value(input.clone())
        .map_err(|e| format!("Failed to convert input to Val: {}", e))?;

    let ctx = Ctx::<D>::new(&filter.lut, Vars::new([]));
    let outputs: Vec<_> = filter.id.run((ctx, val)).collect();

    if outputs.len() != 1 {
        return Err(format!(
            "jq expression must produce exactly one output (got {})",
            outputs.len()
        ));
    }

    let result = outputs
        .into_iter()
        .next()
        .unwrap()
        .map_err(|e| format!("jq runtime error: {e:?}"))?;

    jq_val_to_json(&result)
}

#[cfg(feature = "cli")]
fn jq_run_all(
    filter: &CompiledFilter,
    input: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, String> {
    use jaq_core::{Ctx, Vars};
    use jaq_json::Val;

    let val: Val = serde_json::from_value(input.clone())
        .map_err(|e| format!("Failed to convert input to Val: {}", e))?;

    let ctx = Ctx::<D>::new(&filter.lut, Vars::new([]));
    let mut results = Vec::new();
    for output in filter.id.run((ctx, val)) {
        match output {
            Ok(v) => results.push(jq_val_to_json(&v)?),
            Err(e) => return Err(format!("jq runtime error: {e:?}")),
        }
    }
    Ok(results)
}

/// Produce a jq-facing JSON representation of a unit.
///
/// `links` is always embedded inside `metadata` (as an array, possibly empty) so that jq
/// expressions like `.metadata.links[].to` work without null-guards on units without links.
/// This is consistent with the user-facing API and the on-disk YAML frontmatter format.
#[cfg(feature = "cli")]
fn unit_to_jq_json(unit: &Unit) -> Result<serde_json::Value, String> {
    let mut meta = match &unit.metadata {
        serde_json::Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    let links_json = serde_json::to_value(&unit.links)
        .map_err(|e| format!("Failed to serialize links: {}", e))?;
    meta.insert("links".to_string(), links_json);
    Ok(serde_json::json!({
        "id": unit.id,
        "metadata": serde_json::Value::Object(meta),
        "body": unit.body,
    }))
}

/// Parse a jq-facing JSON representation back into a `Unit`.
///
/// Extracts `links` from inside `metadata` (where jq expressions put them).
#[cfg(feature = "cli")]
fn jq_json_to_unit(value: serde_json::Value) -> Result<Unit, String> {
    let obj = match value {
        serde_json::Value::Object(m) => m,
        other => {
            return Err(format!(
                "jq expression must return an object (got {})",
                other
            ));
        }
    };

    let id = match obj.get("id") {
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => {
            return Err("jq result must have a string .id field".to_string());
        }
    };

    let body = match obj.get("body") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => return Err(format!(".body must be a string, got {other}")),
        None => String::new(),
    };

    let mut metadata = match obj.get("metadata") {
        Some(serde_json::Value::Object(m)) => m.clone(),
        Some(serde_json::Value::Null) | None => serde_json::Map::new(),
        Some(other) => {
            return Err(format!(".metadata must be an object, got {other}"));
        }
    };

    let links = match metadata.remove("links") {
        None | Some(serde_json::Value::Null) => vec![],
        Some(v) => serde_json::from_value(v).map_err(|e| {
            format!(
                ".metadata.links must be an array of {{kind, to}} objects: {}",
                e
            )
        })?,
    };

    Ok(Unit {
        id,
        metadata: serde_json::Value::Object(metadata),
        links,
        body,
    })
}

/// Apply a jq expression to a unit (serialized as JSON).
///
/// The unit is presented to jq with `links` inside `metadata`, so expressions
/// like `.metadata.links += [...]` work as expected.
///
/// Returns:
/// - `Ok(Some(unit))` — transform returned a unit-shaped object.
/// - `Ok(None)` — transform returned `null` (delete semantics).
/// - `Err(msg)` — parse/eval error or result was not an object or null.
#[cfg(feature = "cli")]
pub fn apply_jq_transform(unit: &Unit, expr: &str) -> Result<Option<Unit>, String> {
    let filter = jq_compile(expr)?;
    let unit_json = unit_to_jq_json(unit)?;
    let result = jq_run_one(&filter, &unit_json)?;

    match result {
        serde_json::Value::Null => Ok(None),
        other => Ok(Some(jq_json_to_unit(other)?)),
    }
}

/// Evaluate a jq predicate against a unit, returning true if the predicate is truthy.
#[cfg(feature = "cli")]
pub fn eval_jq_predicate(unit: &Unit, predicate: &str) -> Result<bool, String> {
    use jaq_core::{Ctx, ValT, Vars};
    use jaq_json::Val;

    let filter = jq_compile(predicate)?;
    let unit_json = unit_to_jq_json(unit)?;
    let val: Val = serde_json::from_value(unit_json)
        .map_err(|e| format!("Failed to convert unit to Val: {}", e))?;

    let ctx = Ctx::<D>::new(&filter.lut, Vars::new([]));
    for v in filter.id.run((ctx, val)).flatten() {
        if v.as_bool() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Walk the graph from `start_id`, extracting next-hop IDs from each unit using `link_expr`.
///
/// `link_expr` is a jq expression evaluated against the whole unit JSON; each string output
/// is treated as a unit ID to visit next. Traversal is BFS, de-duped by ID.
/// `depth` limits hops (0 = unlimited). `include_start` controls whether the start unit
/// appears in the results.
#[cfg(feature = "cli")]
pub fn walk_from(
    kg_dir: &Path,
    start_id: &str,
    link_expr: &str,
    depth: usize,
    include_start: bool,
) -> Result<Vec<Unit>, String> {
    use std::collections::{HashSet, VecDeque};

    let filter = jq_compile(link_expr)?;

    let extract_ids = |unit: &Unit| -> Result<Vec<String>, String> {
        let unit_json = unit_to_jq_json(unit)?;
        let outputs = jq_run_all(&filter, &unit_json)?;
        let mut ids = Vec::new();
        for v in outputs {
            if let serde_json::Value::String(s) = v {
                ids.push(s);
            }
        }
        Ok(ids)
    };

    let start_unit =
        read_unit(kg_dir, start_id)?.ok_or_else(|| format!("Unit '{}' not found.", start_id))?;

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(start_id.to_string());

    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let initial_ids = extract_ids(&start_unit)?;
    for id in initial_ids {
        if !visited.contains(&id) {
            visited.insert(id.clone());
            queue.push_back((id, 1));
        }
    }

    let mut result: Vec<Unit> = Vec::new();
    if include_start {
        result.push(start_unit);
    }

    while let Some((id, hop)) = queue.pop_front() {
        let unit = match read_unit(kg_dir, &id)? {
            Some(u) => u,
            None => continue,
        };

        if depth == 0 || hop < depth {
            let next_ids = extract_ids(&unit)?;
            for next_id in next_ids {
                if !visited.contains(&next_id) {
                    visited.insert(next_id.clone());
                    queue.push_back((next_id, hop + 1));
                }
            }
        }

        result.push(unit);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Frontmatter parsing / rendering
// ---------------------------------------------------------------------------

fn parse_unit_file(contents: &str) -> Result<(serde_json::Value, Vec<Link>, String), String> {
    if !contents.starts_with("---") {
        return Ok((
            serde_json::Value::Object(Default::default()),
            vec![],
            contents.to_string(),
        ));
    }

    let after_first = &contents[3..];
    let Some(close_idx) = after_first.find("\n---") else {
        return Ok((
            serde_json::Value::Object(Default::default()),
            vec![],
            contents.to_string(),
        ));
    };

    let yaml_str = &after_first[..close_idx];
    let rest = &after_first[close_idx + 4..];
    let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();

    let yaml_val: serde_yaml::Value =
        serde_yaml::from_str(yaml_str).unwrap_or(serde_yaml::Value::Mapping(Default::default()));

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

fn render_unit_file(metadata: &serde_json::Value, links: &[Link], body: &str) -> String {
    let mut yaml_map = serde_yaml::Mapping::new();

    if let serde_json::Value::Object(meta_map) = metadata {
        for (k, v) in meta_map {
            yaml_map.insert(serde_yaml::Value::String(k.clone()), json_to_yaml_value(v));
        }
    }

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

pub fn migrate_jsonl_if_present(kg_dir: &Path) -> Result<(), String> {
    let edges_path = kg_dir.join("edges.jsonl");
    if !edges_path.exists() {
        return Ok(());
    }

    let edges = project_legacy_edges(kg_dir)?;
    let count = edges.len();

    for edge in edges {
        if let Some(mut unit) = read_unit(kg_dir, &edge.from)? {
            unit.links
                .retain(|l| !(l.kind == edge.kind && l.to == edge.to));
            unit.links.push(Link {
                kind: edge.kind,
                to: edge.to,
                metadata: edge.metadata,
            });
            write_unit(kg_dir, &unit)?;
        }
    }

    let migrated_path = kg_dir.join("edges.jsonl.migrated-v0");
    std::fs::rename(&edges_path, &migrated_path)
        .map_err(|e| format!("Failed to rename edges.jsonl: {}", e))?;

    eprintln!(
        "kg: migrated {} edges from edges.jsonl to unit frontmatter (legacy log renamed to edges.jsonl.migrated-v0)",
        count
    );

    Ok(())
}

struct LegacyEdge {
    from: String,
    to: String,
    kind: String,
    metadata: serde_json::Value,
}

fn project_legacy_edges(kg_dir: &Path) -> Result<Vec<LegacyEdge>, String> {
    use std::collections::HashMap;
    use std::io::BufRead;

    let path = kg_dir.join("edges.jsonl");
    if !path.exists() {
        return Ok(vec![]);
    }

    let mut present: HashMap<(String, String, String), LegacyEdge> = HashMap::new();
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
                    LegacyEdge {
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
            _ => {}
        }
    }

    Ok(order
        .into_iter()
        .filter_map(|k| present.remove(&k))
        .collect())
}

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
    fn test_migration_from_jsonl() {
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

        let edges_path = kg.join("edges.jsonl");
        let lines = [
            r#"{"op":"add","from":"a","to":"b","kind":"ref","metadata":null,"created":"2026-01-01T00:00:00Z"}"#,
            r#"{"op":"add","from":"a","to":"c","kind":"ref","metadata":null,"created":"2026-01-01T00:00:00Z"}"#,
            r#"{"op":"add","from":"b","to":"c","kind":"uses","metadata":{"note":"test"},"created":"2026-01-01T00:00:00Z"}"#,
            r#"{"op":"remove","from":"a","to":"b","kind":"ref","metadata":null,"created":"2026-01-01T00:00:00Z"}"#,
        ];
        std::fs::write(&edges_path, lines.join("\n") + "\n").unwrap();

        migrate_jsonl_if_present(kg).unwrap();

        assert!(!edges_path.exists(), "edges.jsonl should be gone");
        assert!(
            kg.join("edges.jsonl.migrated-v0").exists(),
            "migrated file should exist"
        );

        let a = read_unit(kg, "a").unwrap().unwrap();
        assert_eq!(a.links.len(), 1);
        assert_eq!(a.links[0].to, "c");

        let b = read_unit(kg, "b").unwrap().unwrap();
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.links[0].to, "c");
        assert_eq!(b.links[0].metadata["note"], "test");
    }
}
