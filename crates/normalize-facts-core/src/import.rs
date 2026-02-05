//! Import and export types for code facts.

use serde::{Deserialize, Serialize};

use crate::SymbolKind;

/// An import statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub names: Vec<String>,
    pub alias: Option<String>,
    pub is_wildcard: bool,
    pub is_relative: bool,
    pub line: usize,
}

impl Import {
    /// Format as a readable summary (module + names)
    pub fn format_summary(&self) -> String {
        if self.is_wildcard {
            format!("{}::*", self.module)
        } else if self.names.is_empty() {
            self.module.clone()
        } else if self.names.len() == 1 {
            format!("{}::{}", self.module, self.names[0])
        } else {
            format!("{}::{{{}}}", self.module, self.names.join(", "))
        }
    }
}

/// An export declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
}

/// A flattened import for indexing (one entry per imported name)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatImport {
    /// The module being imported from (None for "import X")
    pub module: Option<String>,
    /// The name being imported
    pub name: String,
    /// Alias if present (from X import Y as Z -> alias = Z)
    pub alias: Option<String>,
    /// Line number
    pub line: usize,
}
