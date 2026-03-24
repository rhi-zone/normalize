//! Shared entity types and ranking infrastructure for analyze commands.
//!
//! Provides the [`Entity`] trait for items that appear in ranked lists,
//! concrete entity types ([`FunctionEntity`], [`ModuleEntity`], [`FileEntity`]),
//! and the [`rank_pipeline`] for shared sort/stats/truncate logic.

pub mod ranked;

use schemars::JsonSchema;
use serde::Serialize;

/// An entity that can appear in a ranked list.
pub trait Entity: Serialize + JsonSchema + Clone {
    /// Display label for this entity (function name, module path, file path).
    fn label(&self) -> &str;
}

/// A function-level entity (for complexity, length, etc.).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FunctionEntity {
    pub name: String,
    pub parent: Option<String>,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl Entity for FunctionEntity {
    fn label(&self) -> &str {
        &self.name
    }
}

/// A module-level entity (for density, health, etc.).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ModuleEntity {
    pub path: String,
}

impl Entity for ModuleEntity {
    fn label(&self) -> &str {
        &self.path
    }
}

/// A file-level entity (for file-scoped rankings).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileEntity {
    pub path: String,
}

impl Entity for FileEntity {
    fn label(&self) -> &str {
        &self.path
    }
}

/// Truncate a path for display, replacing the beginning with "..." if too long.
///
/// Used by many ranked-list formatters to keep tabular output aligned.
pub fn truncate_path(path: &str, max_len: usize) -> String {
    if max_len <= 3 {
        return path.to_string();
    }
    if path.len() > max_len {
        let target = path.len().saturating_sub(max_len - 3);
        let safe_start = path
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= target)
            .unwrap_or(path.len());
        format!("...{}", &path[safe_start..])
    } else {
        path.to_string()
    }
}
