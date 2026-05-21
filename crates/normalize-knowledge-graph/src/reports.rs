//! Report structs for normalize kg commands.
//!
//! All structs implement `OutputFormatter` (gated by the `cli` feature).

use crate::model::Unit;
use serde::Serialize;

#[cfg(feature = "cli")]
use schemars::JsonSchema;

// ---------------------------------------------------------------------------
// UnitReport
// ---------------------------------------------------------------------------

/// Report for a single unit.
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
// ReadReport
// ---------------------------------------------------------------------------

/// Report for `normalize kg read` (one or many units).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct ReadReport {
    pub units: Vec<UnitReport>,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for ReadReport {
    fn format_text(&self) -> String {
        if self.units.is_empty() {
            return "No units.\n".to_string();
        }
        let mut out = String::new();
        for (i, unit) in self.units.iter().enumerate() {
            if i > 0 {
                out.push_str("---\n");
            }
            out.push_str(&unit.format_text());
        }
        out
    }
}

// ---------------------------------------------------------------------------
// WriteReport
// ---------------------------------------------------------------------------

/// Report for `normalize kg write`.
///
/// `unit` is `None` when the transform returned null (unit was deleted).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct WriteReport {
    pub unit: Option<UnitReport>,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for WriteReport {
    fn format_text(&self) -> String {
        match &self.unit {
            Some(u) => u.format_text(),
            None => "Deleted.\n".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// WalkReport
// ---------------------------------------------------------------------------

/// Report for `normalize kg walk`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "cli", derive(JsonSchema))]
pub struct WalkReport {
    pub units: Vec<UnitReport>,
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for WalkReport {
    fn format_text(&self) -> String {
        if self.units.is_empty() {
            return "No units reachable.\n".to_string();
        }
        let mut out = String::new();
        for (i, unit) in self.units.iter().enumerate() {
            if i > 0 {
                out.push_str("---\n");
            }
            out.push_str(&unit.format_text());
        }
        out
    }
}
