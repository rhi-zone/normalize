//! CLI service for normalize kg subcommands.
//!
//! Exposes three primitives: read, write, walk.

use crate::model::Unit;
use crate::reports::{ReadReport, UnitReport, WalkReport, WriteReport};
use crate::store;
use normalize_output::OutputFormatter;
use server_less::cli;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_root(root: Option<String>) -> Result<PathBuf, String> {
    root.map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|e| format!("Failed to get current directory: {}", e))
}

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

fn open_kg(root: &Path) -> Result<PathBuf, String> {
    let norm_dir = get_normalize_dir(root);
    let kg = store::ensure_kg_dir(&norm_dir)?;
    store::migrate_jsonl_if_present(&kg)?;
    Ok(kg)
}

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
    /// Read units from the knowledge graph.
    ///
    /// Without a selector, lists all units. With an ID, returns that unit.
    /// With -q, scans all units and returns those where the predicate is truthy.
    ///
    /// Examples:
    ///   normalize kg read
    ///   normalize kg read my-design
    ///   normalize kg read -q '.metadata.tag == "design"'
    #[cli(display_with = "display_output")]
    pub fn read(
        &self,
        #[param(positional, help = "Unit ID (exact lookup)")] id: Option<String>,
        #[param(
            short = 'q',
            help = "jq predicate — scan all units where predicate is truthy"
        )]
        query: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<ReadReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        if let Some(id) = id {
            let unit =
                store::read_unit(&kg, &id)?.ok_or_else(|| format!("Unit '{}' not found.", id))?;
            return Ok(ReadReport {
                units: vec![UnitReport::from_unit(&unit)],
            });
        }

        if let Some(predicate) = query {
            let all = store::read_all_units(&kg)?;
            let mut units = Vec::new();
            for unit in &all {
                if store::eval_jq_predicate(unit, &predicate)? {
                    units.push(UnitReport::from_unit(unit));
                }
            }
            return Ok(ReadReport { units });
        }

        let all = store::read_all_units(&kg)?;
        Ok(ReadReport {
            units: all.iter().map(UnitReport::from_unit).collect(),
        })
    }

    /// Write a unit to the knowledge graph.
    ///
    /// Without a selector, reads a JSON unit from stdin and stores it (.id is the key).
    /// With an ID and a jq transform, applies the transform to the existing unit:
    ///   - transform returns an object → stored as the new unit
    ///   - transform returns null → unit is deleted
    ///
    /// Examples:
    ///   echo '{"id":"a","metadata":{},"body":"Hello"}' | normalize kg write
    ///   normalize kg write my-design '.metadata.tag = "approved"'
    ///   normalize kg write my-design 'null'
    ///   normalize kg write my-design 'null' --dry-run   # preview the delete
    #[cli(display_with = "display_output")]
    pub fn write(
        &self,
        #[param(positional, help = "Unit ID to mutate")] id: Option<String>,
        #[param(positional, help = "jq transform expression")] transform: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Dry run - show what would change without writing")] dry_run: bool,
    ) -> Result<WriteReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        match (id, transform) {
            (None, _) => {
                let raw = read_stdin()?;
                let unit: Unit = serde_json::from_str(&raw).map_err(|e| {
                    format!(
                        "stdin must be a JSON unit (with id, metadata, body): {}",
                        e
                    )
                })?;
                if unit.id.is_empty() {
                    return Err("Unit JSON must have a non-empty .id field.".to_string());
                }
                if !dry_run {
                    store::write_unit(&kg, &unit)?;
                }
                Ok(WriteReport {
                    unit: Some(UnitReport::from_unit(&unit)),
                    dry_run,
                })
            }
            (Some(id), Some(expr)) => {
                let existing = store::read_unit(&kg, &id)?
                    .ok_or_else(|| format!("Unit '{}' not found.", id))?;
                match store::apply_jq_transform(&existing, &expr)? {
                    None => {
                        if !dry_run {
                            store::delete_unit(&kg, &id)?;
                        }
                        Ok(WriteReport {
                            unit: None,
                            dry_run,
                        })
                    }
                    Some(updated) => {
                        if !dry_run {
                            store::write_unit(&kg, &updated)?;
                        }
                        Ok(WriteReport {
                            unit: Some(UnitReport::from_unit(&updated)),
                            dry_run,
                        })
                    }
                }
            }
            (Some(_), None) => Err(
                "kg write <id> requires a jq transform expression. To delete, use: kg write <id> 'null'".to_string(),
            ),
        }
    }

    /// Walk the graph from a starting unit, following links extracted by a jq expression.
    ///
    /// The jq expression is applied to each unit's full JSON; each string output is treated
    /// as a unit ID to visit next. Traversal is BFS, de-duped by ID.
    ///
    /// Examples:
    ///   normalize kg walk my-design '.metadata.links[].to'
    ///   normalize kg walk my-design '.metadata.links[].to' --depth 2
    ///   normalize kg walk my-design '.metadata.links[].to' --include-start
    #[cli(display_with = "display_output")]
    pub fn walk(
        &self,
        #[param(positional, help = "Starting unit ID")] id: String,
        #[param(
            positional,
            help = "jq expression extracting link target IDs from each unit"
        )]
        link_expr: String,
        #[param(short = 'd', help = "Maximum hop depth (0 = unlimited, default)")] depth: Option<
            usize,
        >,
        #[param(help = "Include the starting unit in the output (default off)")]
        include_start: bool,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<WalkReport, String> {
        let root_path = resolve_root(root)?;
        let kg = open_kg(&root_path)?;

        let depth = depth.unwrap_or(0);
        let units = store::walk_from(&kg, &id, &link_expr, depth, include_start)?;
        Ok(WalkReport {
            units: units.iter().map(UnitReport::from_unit).collect(),
        })
    }
}
