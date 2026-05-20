//! Report structs for normalize kg commands.
//!
//! All structs implement `OutputFormatter` (gated by the `cli` feature).

use crate::model::{Edge, Unit};
use serde::Serialize;

// ---------------------------------------------------------------------------
// UnitReport
// ---------------------------------------------------------------------------

/// Report for a single unit (create, get, set, append).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct UnitReport {
    pub id: String,
    pub metadata: serde_json::Value,
    pub body: String,
}

impl UnitReport {
    pub fn from_unit(unit: &Unit) -> Self {
        Self {
            id: unit.id.clone(),
            metadata: unit.metadata.clone(),
            body: unit.body.clone(),
        }
    }
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for UnitReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("id: {}\n", self.id));
        if !self.metadata.is_null()
            && self.metadata != serde_json::Value::Object(Default::default())
        {
            out.push_str("metadata:\n");
            if let Some(map) = self.metadata.as_object() {
                for (k, v) in map {
                    out.push_str(&format!("  {}: {}\n", k, v));
                }
            }
        }
        if !self.body.is_empty() {
            out.push('\n');
            out.push_str(self.body.trim_end());
            out.push('\n');
        }
        out
    }
}

// ---------------------------------------------------------------------------
// DeleteReport
// ---------------------------------------------------------------------------

/// Report for `normalize kg delete`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct DeleteReport {
    pub id: String,
    pub deleted: bool,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for DeleteReport {
    fn format_text(&self) -> String {
        if self.deleted {
            format!("Deleted: {}\n", self.id)
        } else {
            format!("Not found: {}\n", self.id)
        }
    }
}

// ---------------------------------------------------------------------------
// EdgeReport
// ---------------------------------------------------------------------------

/// Report for a single edge.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct EdgeReport {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub metadata: serde_json::Value,
}

impl EdgeReport {
    pub fn from_edge(edge: &Edge) -> Self {
        Self {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: edge.kind.clone(),
            metadata: edge.metadata.clone(),
        }
    }
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for EdgeReport {
    fn format_text(&self) -> String {
        format!("{} --[{}]--> {}\n", self.from, self.kind, self.to)
    }
}

// ---------------------------------------------------------------------------
// EdgeListReport
// ---------------------------------------------------------------------------

/// Report for `normalize kg edges`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct EdgeListReport {
    pub edges: Vec<EdgeReport>,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for EdgeListReport {
    fn format_text(&self) -> String {
        if self.edges.is_empty() {
            return "No edges found.\n".to_string();
        }
        use normalize_output::OutputFormatter as _;
        self.edges.iter().map(|e| e.format_text()).collect()
    }
}

// ---------------------------------------------------------------------------
// QueryReport
// ---------------------------------------------------------------------------

/// Report for `normalize kg query`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct QueryReport {
    pub units: Vec<UnitReport>,
    pub total: usize,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for QueryReport {
    fn format_text(&self) -> String {
        if self.units.is_empty() {
            return "No matching units.\n".to_string();
        }
        use normalize_output::OutputFormatter as _;
        let mut out = String::new();
        for (i, unit) in self.units.iter().enumerate() {
            if i > 0 {
                out.push_str("---\n");
            }
            out.push_str(&unit.format_text());
        }
        out.push_str(&format!("\nTotal: {} unit(s)\n", self.total));
        out
    }
}

// ---------------------------------------------------------------------------
// NeighborsReport
// ---------------------------------------------------------------------------

/// One neighbor entry: the connecting edge and the adjacent unit.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct NeighborEntry {
    pub edge: EdgeReport,
    pub unit: UnitReport,
}

/// Report for `normalize kg neighbors`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct NeighborsReport {
    pub center: UnitReport,
    pub neighbors: Vec<NeighborEntry>,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for NeighborsReport {
    fn format_text(&self) -> String {
        use normalize_output::OutputFormatter as _;
        let mut out = String::new();
        out.push_str(&format!("Center: {}\n", self.center.id));
        if self.neighbors.is_empty() {
            out.push_str("No neighbors.\n");
        } else {
            for entry in &self.neighbors {
                out.push_str(&format!(
                    "  {} --[{}]--> {} ({})\n",
                    entry.edge.from, entry.edge.kind, entry.edge.to, entry.unit.id
                ));
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// ShowReport
// ---------------------------------------------------------------------------

/// Report for `normalize kg show` (unit + neighbors).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct ShowReport {
    pub unit: UnitReport,
    pub neighbors: Vec<NeighborEntry>,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for ShowReport {
    fn format_text(&self) -> String {
        use normalize_output::OutputFormatter as _;
        let mut out = self.unit.format_text();
        if !self.neighbors.is_empty() {
            out.push_str("\nNeighbors:\n");
            for entry in &self.neighbors {
                out.push_str(&format!(
                    "  {} --[{}]--> {}\n",
                    entry.edge.from, entry.edge.kind, entry.edge.to
                ));
            }
        }
        out
    }
}
