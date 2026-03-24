//! Import and export types for code facts.

use serde::{Deserialize, Serialize};

use crate::SymbolKind;

/// An import statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    /// The module specifier as written in source (e.g., `"./foo"` or `std::collections`).
    pub module: String,
    /// Specific names imported from the module (e.g., `["HashMap", "HashSet"]`).
    pub names: Vec<String>,
    /// Local alias for the import (e.g., `import numpy as np` → `"np"`).
    pub alias: Option<String>,
    /// True for wildcard imports (`import *` / `use *`).
    pub is_wildcard: bool,
    /// True for relative imports (e.g., `./foo`, `../bar`).
    pub is_relative: bool,
    /// Source line number where this import appears.
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
    /// The exported name as it appears in source.
    pub name: String,
    /// The symbol kind being exported (function, class, variable, etc.).
    pub kind: SymbolKind,
    /// Source line number where this export appears.
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
