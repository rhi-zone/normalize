//! CLI service for normalize kg subcommands.
//!
//! Exposes `kg` subcommands: create, get, set, append, delete, link, unlink,
//! edges, query, neighbors, show.

use crate::model::{Edge, EdgeOp, EdgeOpKind, Unit, validate_id};
use crate::query::{MatchPredicate, bfs_neighbors, filter_edges, filter_units};
use crate::reports::{
    DeleteReport, EdgeListReport, EdgeReport, NeighborEntry, NeighborsReport, QueryReport,
    ShowReport, UnitReport,
};
use crate::store;
use normalize_output::OutputFormatter;
use server_less::cli;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve project root: use provided string or fall back to current directory.
fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
    root.map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {}", e))
}

/// Get the normalize data directory for a project.
///
/// Mirrors the logic in `normalize::paths::get_normalize_dir` without requiring
/// a dependency on the main `normalize` crate.
fn get_normalize_dir(root: &Path) -> PathBuf {
    if let Ok(index_dir) = std::env::var("NORMALIZE_INDEX_DIR") {
        let path = PathBuf::from(&index_dir);
        if path.is_absolute() {
            return path;
        }
        let data_home = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/share")
            });
        return data_home.join("normalize").join(&index_dir);
    }
    root.join(".normalize")
}

/// Get the normalize dir then the kg dir, ensuring it exists.
fn open_kg(root: &Path) -> Result<PathBuf, String> {
    let norm_dir = get_normalize_dir(root);
    store::ensure_kg_dir(&norm_dir)
}

/// Parse `key=value` metadata pairs into a JSON object.
fn parse_metadata_pairs(pairs: &[String]) -> Result<serde_json::Value, String> {
    let mut map = serde_json::Map::new();
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(format!("Invalid metadata '{}': expected 'key=value'", pair));
        };
        // Support dotted keys: "anchors.symbol=Frobnicator" → {"anchors": {"symbol": "..."}}
        insert_dotted(&mut map, key, serde_json::Value::String(value.to_string()));
    }
    Ok(serde_json::Value::Object(map))
}

/// Insert a value at a dotted path into a JSON map, creating intermediate objects.
fn insert_dotted(
    map: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    value: serde_json::Value,
) {
    if let Some((head, tail)) = path.split_once('.') {
        let child = map
            .entry(head.to_string())
            .or_insert_with(|| serde_json::Value::Object(Default::default()));
        if let serde_json::Value::Object(child_map) = child {
            insert_dotted(child_map, tail, value);
        }
    } else {
        map.insert(path.to_string(), value);
    }
}

/// Merge `new_meta` into `base` (top-level key merge, no deep merge).
fn merge_metadata(base: &serde_json::Value, new_meta: &serde_json::Value) -> serde_json::Value {
    let mut result = base.clone();
    if let (Some(base_map), Some(new_map)) = (result.as_object_mut(), new_meta.as_object()) {
        for (k, v) in new_map {
            base_map.insert(k.clone(), v.clone());
        }
    }
    result
}

/// Current ISO 8601 timestamp.
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Read body from stdin.
fn read_stdin() -> Result<String, String> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("Failed to read stdin: {}", e))?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// CLI service for `normalize kg` subcommands.
pub struct KgCliService;

impl KgCliService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KgCliService {
    fn default() -> Self {
        Self::new()
    }
}

impl KgCliService {
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

#[cli(
    name = "normalize-knowledge-graph",
    version = "0.1.0",
    description = "Persistent knowledge graph adjacent to code"
)]
impl KgCliService {
    /// Create a new unit (body via stdin). Auto-generates an ID if --id is not provided.
    ///
    /// Examples:
    ///   echo "Design notes." | normalize kg create --id my-design --metadata tag=design
    #[cli(display_with = "display_output")]
    pub fn create(
        &self,
        #[param(short = 'i', help = "Unit ID (auto-generated if omitted)")] id: Option<String>,
        #[param(short = 'm', help = "Metadata key=value pairs (repeatable)")] metadata: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<UnitReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        // Generate an ID if not provided
        let id = match id {
            Some(id) => {
                validate_id(&id)?;
                id
            }
            None => {
                // Generate a short random-ish ID based on timestamp
                let ts = chrono::Utc::now().timestamp_millis();
                format!("unit-{}", ts)
            }
        };

        // Check for collision
        if store::read_unit(&kg, &id)?.is_some() {
            return Err(format!(
                "Unit '{}' already exists. Use `set` to update.",
                id
            ));
        }

        let meta = parse_metadata_pairs(&metadata)?;
        let body = read_stdin()?;

        let unit = Unit {
            id: id.clone(),
            metadata: meta,
            body,
        };
        store::write_unit(&kg, &unit)?;

        Ok(UnitReport::from_unit(&unit))
    }

    /// Get a unit by ID.
    ///
    /// Examples:
    ///   normalize kg get my-design
    #[cli(display_with = "display_output")]
    pub fn get(
        &self,
        #[param(positional, help = "Unit ID")] id: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<UnitReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&id)?;

        let unit =
            store::read_unit(&kg, &id)?.ok_or_else(|| format!("Unit '{}' not found.", id))?;

        Ok(UnitReport::from_unit(&unit))
    }

    /// Set (merge) metadata on an existing unit.
    ///
    /// Merges provided key=value pairs into the existing frontmatter.
    ///
    /// Examples:
    ///   normalize kg set my-design --metadata status=approved
    #[cli(display_with = "display_output")]
    pub fn set(
        &self,
        #[param(positional, help = "Unit ID")] id: String,
        #[param(short = 'm', help = "Metadata key=value pairs to merge")] metadata: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<UnitReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&id)?;

        let mut unit =
            store::read_unit(&kg, &id)?.ok_or_else(|| format!("Unit '{}' not found.", id))?;

        let new_meta = parse_metadata_pairs(&metadata)?;
        unit.metadata = merge_metadata(&unit.metadata, &new_meta);

        store::write_unit(&kg, &unit)?;

        Ok(UnitReport::from_unit(&unit))
    }

    /// Append body text to an existing unit (reads from stdin).
    ///
    /// Appends text after a blank line separator.
    ///
    /// Examples:
    ///   echo "Additional notes." | normalize kg append my-design
    #[cli(display_with = "display_output")]
    pub fn append(
        &self,
        #[param(positional, help = "Unit ID")] id: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<UnitReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&id)?;

        let mut unit =
            store::read_unit(&kg, &id)?.ok_or_else(|| format!("Unit '{}' not found.", id))?;

        let new_text = read_stdin()?;
        if !new_text.is_empty() {
            if !unit.body.is_empty() && !unit.body.ends_with('\n') {
                unit.body.push('\n');
            }
            unit.body.push('\n');
            unit.body.push_str(&new_text);
        }

        store::write_unit(&kg, &unit)?;

        Ok(UnitReport::from_unit(&unit))
    }

    /// Delete a unit by ID.
    ///
    /// Examples:
    ///   normalize kg delete my-design
    #[cli(display_with = "display_output")]
    pub fn delete(
        &self,
        #[param(positional, help = "Unit ID")] id: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<DeleteReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&id)?;

        let deleted = store::delete_unit(&kg, &id)?;

        Ok(DeleteReport { id, deleted })
    }

    /// Add a directed edge between two units.
    ///
    /// Examples:
    ///   normalize kg link --from design-doc --to api-spec --kind references
    #[cli(display_with = "display_output")]
    pub fn link(
        &self,
        #[param(short = 'f', help = "Source unit ID")] from: String,
        #[param(short = 't', help = "Target unit ID")] to: String,
        #[param(short = 'k', help = "Edge kind label")] kind: String,
        #[param(short = 'm', help = "Metadata key=value pairs")] metadata: Vec<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<EdgeReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&from)?;
        validate_id(&to)?;

        let meta = parse_metadata_pairs(&metadata)?;

        let op = EdgeOp {
            op: EdgeOpKind::Add,
            from: from.clone(),
            to: to.clone(),
            kind: kind.clone(),
            metadata: meta.clone(),
            created: now_iso(),
        };
        store::append_edge_op(&kg, &op)?;

        Ok(EdgeReport {
            from,
            to,
            kind,
            metadata: meta,
        })
    }

    /// Remove a directed edge between two units.
    ///
    /// Appends a tombstone to edges.jsonl; does not modify history.
    ///
    /// Examples:
    ///   normalize kg unlink --from design-doc --to api-spec --kind references
    #[cli(display_with = "display_output")]
    pub fn unlink(
        &self,
        #[param(short = 'f', help = "Source unit ID")] from: String,
        #[param(short = 't', help = "Target unit ID")] to: String,
        #[param(short = 'k', help = "Edge kind label")] kind: String,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<EdgeReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&from)?;
        validate_id(&to)?;

        let op = EdgeOp {
            op: EdgeOpKind::Remove,
            from: from.clone(),
            to: to.clone(),
            kind: kind.clone(),
            metadata: serde_json::Value::Null,
            created: now_iso(),
        };
        store::append_edge_op(&kg, &op)?;

        Ok(EdgeReport {
            from,
            to,
            kind,
            metadata: serde_json::Value::Null,
        })
    }

    /// List current (projected) edges, with optional filters.
    ///
    /// Examples:
    ///   normalize kg edges --from design-doc
    ///   normalize kg edges --kind references
    #[cli(display_with = "display_output")]
    pub fn edges(
        &self,
        #[param(short = 'f', help = "Filter by source unit ID")] from: Option<String>,
        #[param(short = 't', help = "Filter by target unit ID")] to: Option<String>,
        #[param(short = 'k', help = "Filter by edge kind")] kind: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<EdgeListReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        let all_edges = store::project_edges(&kg)?;
        let filtered = filter_edges(
            all_edges.iter(),
            from.as_deref(),
            to.as_deref(),
            kind.as_deref(),
        );

        Ok(EdgeListReport {
            edges: filtered.iter().map(|e| EdgeReport::from_edge(e)).collect(),
        })
    }

    /// Query units by metadata, edge-kind, or connected-to.
    ///
    /// All filters are ANDed together.
    ///
    /// Examples:
    ///   normalize kg query --match tag=wiki-page
    ///   normalize kg query --match anchors.symbol=Frobnicator
    ///   normalize kg query --connected-to my-design
    #[cli(display_with = "display_output")]
    pub fn query(
        &self,
        #[param(
            short = 'm',
            help = "Match key=value predicates (dotted-path, repeatable)"
        )]
        r#match: Vec<String>,
        #[param(short = 'k', help = "Filter units connected by this edge kind")] edge_kind: Option<
            String,
        >,
        #[param(short = 'c', help = "Filter units connected to this unit ID")] connected_to: Option<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<QueryReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        let predicates: Vec<MatchPredicate> = r#match
            .iter()
            .map(|s| MatchPredicate::parse(s))
            .collect::<Result<Vec<_>, _>>()?;

        let all_units = store::read_all_units(&kg)?;
        let all_edges = store::project_edges(&kg)?;

        // Apply edge-kind filter if provided
        let filtered_edges: Vec<Edge> = if let Some(ref k) = edge_kind {
            all_edges.iter().filter(|e| &e.kind == k).cloned().collect()
        } else {
            all_edges.clone()
        };

        let matched = filter_units(
            all_units.iter(),
            &predicates,
            connected_to.as_deref(),
            &filtered_edges,
        );

        let total = matched.len();
        let units = matched.iter().map(|u| UnitReport::from_unit(u)).collect();

        Ok(QueryReport { units, total })
    }

    /// List units neighboring a given unit (BFS up to depth N).
    ///
    /// Returns outgoing AND incoming edges at each hop.
    ///
    /// Examples:
    ///   normalize kg neighbors my-design
    ///   normalize kg neighbors my-design --depth 2
    #[cli(display_with = "display_output")]
    pub fn neighbors(
        &self,
        #[param(positional, help = "Center unit ID")] id: String,
        #[param(short = 'd', help = "BFS depth (default 1)")] depth: Option<usize>,
        #[param(short = 'k', help = "Filter traversal to this edge kind")] edge_kind: Option<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<NeighborsReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&id)?;

        let center_unit =
            store::read_unit(&kg, &id)?.ok_or_else(|| format!("Unit '{}' not found.", id))?;

        let all_units = store::read_all_units(&kg)?;
        let all_edges = store::project_edges(&kg)?;

        let depth = depth.unwrap_or(1);
        let neighbor_pairs =
            bfs_neighbors(&id, depth, &all_edges, &all_units, edge_kind.as_deref());

        let neighbors = neighbor_pairs
            .iter()
            .map(|(e, u)| NeighborEntry {
                edge: EdgeReport::from_edge(e),
                unit: UnitReport::from_unit(u),
            })
            .collect();

        Ok(NeighborsReport {
            center: UnitReport::from_unit(&center_unit),
            neighbors,
        })
    }

    /// Show a unit and its neighbors (convenience: unit + neighbors at depth N).
    ///
    /// Examples:
    ///   normalize kg show my-design
    ///   normalize kg show my-design --depth 2
    #[cli(display_with = "display_output")]
    pub fn show(
        &self,
        #[param(positional, help = "Unit ID")] id: String,
        #[param(short = 'd', help = "Neighbor depth (default 1)")] depth: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<ShowReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        validate_id(&id)?;

        let unit =
            store::read_unit(&kg, &id)?.ok_or_else(|| format!("Unit '{}' not found.", id))?;

        let all_units = store::read_all_units(&kg)?;
        let all_edges = store::project_edges(&kg)?;

        let depth = depth.unwrap_or(1);
        let neighbor_pairs = bfs_neighbors(&id, depth, &all_edges, &all_units, None);

        let neighbors = neighbor_pairs
            .iter()
            .map(|(e, u)| NeighborEntry {
                edge: EdgeReport::from_edge(e),
                unit: UnitReport::from_unit(u),
            })
            .collect();

        Ok(ShowReport {
            unit: UnitReport::from_unit(&unit),
            neighbors,
        })
    }
}
