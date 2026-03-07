//! Edit command types — shared between service/edit.rs and the rest of the crate.

/// Position for insert/move/copy operations
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    /// Before the destination (sibling)
    Before,
    /// After the destination (sibling)
    After,
    /// At start of container
    Prepend,
    /// At end of container
    Append,
}

impl std::str::FromStr for Position {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "before" => Ok(Position::Before),
            "after" => Ok(Position::After),
            "prepend" => Ok(Position::Prepend),
            "append" => Ok(Position::Append),
            _ => Err(format!(
                "Unknown position: {} (expected: before, after, prepend, append)",
                s
            )),
        }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Position::Before => write!(f, "before"),
            Position::After => write!(f, "after"),
            Position::Prepend => write!(f, "prepend"),
            Position::Append => write!(f, "append"),
        }
    }
}

/// Internal representation of operations (for output)
#[derive(Clone, Copy)]
pub enum Operation {
    Delete,
    Replace,
    Swap,
    Insert(Position),
    Move(Position),
    Copy(Position),
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Delete => write!(f, "delete"),
            Operation::Replace => write!(f, "replace"),
            Operation::Swap => write!(f, "swap"),
            Operation::Insert(Position::Before) => write!(f, "insert_before"),
            Operation::Insert(Position::After) => write!(f, "insert_after"),
            Operation::Insert(Position::Prepend) => write!(f, "prepend"),
            Operation::Insert(Position::Append) => write!(f, "append"),
            Operation::Move(Position::Before) => write!(f, "move_before"),
            Operation::Move(Position::After) => write!(f, "move_after"),
            Operation::Move(Position::Prepend) => write!(f, "move_prepend"),
            Operation::Move(Position::Append) => write!(f, "move_append"),
            Operation::Copy(Position::Before) => write!(f, "copy_before"),
            Operation::Copy(Position::After) => write!(f, "copy_after"),
            Operation::Copy(Position::Prepend) => write!(f, "copy_prepend"),
            Operation::Copy(Position::Append) => write!(f, "copy_append"),
        }
    }
}

/// Edit action to perform (CLI)
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub enum EditAction {
    /// Delete the target symbol
    Delete,

    /// Replace target with new content
    Replace {
        /// New content to replace with
        content: String,
    },

    /// Swap target with another symbol
    Swap {
        /// Symbol to swap with
        other: String,
    },

    /// Insert content relative to target
    Insert {
        /// Content to insert
        content: String,
        /// Where to insert: before, after, prepend, append
        at: Position,
    },

    /// Move target to a new location
    Move {
        /// Destination symbol or container
        destination: String,
        /// Where to place: before, after, prepend, append
        at: Position,
    },

    /// Copy target to a new location
    Copy {
        /// Destination symbol or container
        destination: String,
        /// Where to place: before, after, prepend, append
        at: Position,
    },
}

/// A single undo/redo/goto change entry, for JSON output.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct EditChange {
    pub description: String,
    pub commit: String,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub conflicts: Vec<String>,
}

/// Structured result for all edit operations (delete, replace, undo, batch, etc.).
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct EditResult {
    pub success: bool,
    pub operation: String,
    /// Relative path of the edited file (structural edits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Symbol that was edited (structural edits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Whether this was a dry run (no files changed).
    pub dry_run: bool,
    /// New file content, present only on dry-run structural edits.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_content: Option<String>,
    /// Undo/redo/goto changes.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub changes: Vec<EditChange>,
    /// Files modified (batch edits).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub files: Vec<String>,
}

impl std::fmt::Display for EditResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Multi-file result (--each or batch) — handled same for dry-run and actual
        if !self.files.is_empty() {
            let prefix = if let Some(ref sym) = self.symbol {
                format!("{} in {} file(s)", sym, self.files.len())
            } else {
                format!("{} file(s)", self.files.len())
            };
            if self.dry_run {
                write!(f, "Would {}: {}", self.operation, prefix)?;
            } else {
                write!(f, "{}: {}", self.operation, prefix)?;
            }
            for file in &self.files {
                write!(f, "\n  {}", file)?;
            }
            return Ok(());
        }

        if self.dry_run {
            if let Some(ref content) = self.new_content {
                if let Some(ref sym) = self.symbol {
                    writeln!(f, "--- Dry run: {} on {} ---", self.operation, sym)?;
                } else if let Some(ref file) = self.file {
                    writeln!(f, "--- Dry run: {} on {} ---", self.operation, file)?;
                }
                write!(f, "{}", content)?;
                return Ok(());
            }
            // undo/goto dry run
            for change in &self.changes {
                writeln!(
                    f,
                    "Would {}: {} ({})",
                    self.operation, change.description, change.commit
                )?;
                for file in &change.files {
                    writeln!(f, "  {}", file)?;
                }
            }
            return Ok(());
        }
        // Actual edit
        if !self.changes.is_empty() {
            // undo/redo/goto
            let verb = match self.operation.as_str() {
                "redo" => "Redone",
                "goto" | "goto_preview" => "Restored",
                _ => "Undone",
            };
            writeln!(
                f,
                "{} {} edit{}:",
                verb,
                self.changes.len(),
                if self.changes.len() == 1 { "" } else { "s" }
            )?;
            for change in &self.changes {
                writeln!(f, "  {} ({})", change.description, change.commit)?;
                for file in &change.files {
                    writeln!(f, "    {}", file)?;
                }
                if !change.conflicts.is_empty() {
                    writeln!(f, "    Conflicts (modified externally):")?;
                    for conflict in &change.conflicts {
                        writeln!(f, "      {}", conflict)?;
                    }
                }
            }
            return Ok(());
        }
        // structural edit
        match (&self.symbol, &self.file) {
            (Some(sym), Some(file)) => write!(f, "{}: {} in {}", self.operation, sym, file),
            (None, Some(file)) => write!(f, "{}: {}", self.operation, file),
            _ => write!(f, "{} completed", self.operation),
        }
    }
}
