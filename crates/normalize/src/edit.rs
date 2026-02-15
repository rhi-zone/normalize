use crate::path_resolve;
use std::path::Path;

// Re-export core types from the extracted crate
pub use normalize_edit::{ContainerBody, Editor, SymbolLocation, line_to_byte};

/// Extension methods that depend on CLI-internal modules (path_resolve, parsers)
pub trait EditorExt {
    /// Check if a pattern contains glob characters (delegates to path_resolve)
    fn is_glob_pattern(pattern: &str) -> bool;

    /// Find all symbols matching a glob pattern in their path.
    /// Returns matches sorted by byte offset (reverse order for safe deletion).
    fn find_symbols_matching(
        &self,
        path: &Path,
        content: &str,
        pattern: &str,
    ) -> Vec<SymbolLocation>;
}

impl EditorExt for Editor {
    fn is_glob_pattern(pattern: &str) -> bool {
        path_resolve::is_glob_pattern(pattern)
    }

    fn find_symbols_matching(
        &self,
        path: &Path,
        content: &str,
        pattern: &str,
    ) -> Vec<SymbolLocation> {
        let symbol_matches = path_resolve::resolve_symbol_glob(path, content, pattern);

        let mut locations: Vec<SymbolLocation> = symbol_matches
            .into_iter()
            .map(|m| {
                let start_byte = line_to_byte(content, m.symbol.start_line);
                let end_byte = line_to_byte(content, m.symbol.end_line + 1);
                SymbolLocation {
                    name: m.symbol.name,
                    kind: m.symbol.kind.as_str().to_string(),
                    start_byte,
                    end_byte,
                    start_line: m.symbol.start_line,
                    end_line: m.symbol.end_line,
                    indent: String::new(),
                }
            })
            .collect();

        // Sort by start position (reverse for safe deletion from end to start)
        locations.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));
        locations
    }
}

// ============================================================================
// Batch Edit Support
// ============================================================================

use std::collections::HashMap;
use std::path::PathBuf;

/// Action to perform in a batch edit
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BatchAction {
    /// Delete a symbol
    Delete,
    /// Replace a symbol with new content
    Replace { content: String },
    /// Insert content relative to a symbol
    Insert {
        content: String,
        #[serde(default = "default_position")]
        position: String, // "before", "after", "prepend", "append"
    },
}

fn default_position() -> String {
    "after".to_string()
}

/// A single edit operation in a batch
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BatchEditOp {
    /// Target path (e.g., "src/main.py/foo" or "src/main.py:42")
    pub target: String,
    /// Action to perform
    #[serde(flatten)]
    pub action: BatchAction,
}

/// Result of applying a batch edit
#[derive(Debug)]
pub struct BatchEditResult {
    /// Files that were modified
    pub files_modified: Vec<PathBuf>,
    /// Number of edits applied
    pub edits_applied: usize,
    /// Errors encountered (target -> error message)
    pub errors: Vec<(String, String)>,
}

/// Preview of a file's changes before applying
#[derive(Debug)]
pub struct FilePreview {
    /// Path to the file
    pub path: PathBuf,
    /// Original content
    pub original: String,
    /// Modified content
    pub modified: String,
    /// Number of edits in this file
    pub edit_count: usize,
}

/// Result of previewing a batch edit
#[derive(Debug)]
pub struct BatchPreviewResult {
    /// Previews for each file that would be modified
    pub files: Vec<FilePreview>,
    /// Total number of edits
    pub total_edits: usize,
}

/// Batch editor for atomic multi-file edits
pub struct BatchEdit {
    edits: Vec<BatchEditOp>,
    message: Option<String>,
}

impl BatchEdit {
    /// Create a new batch edit
    pub fn new() -> Self {
        Self {
            edits: Vec::new(),
            message: None,
        }
    }

    /// Create batch edit from JSON
    pub fn from_json(json: &str) -> Result<Self, String> {
        let edits: Vec<BatchEditOp> =
            serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {}", e))?;
        Ok(Self {
            edits,
            message: None,
        })
    }

    /// Set the commit message for shadow git
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }

    /// Add an edit operation
    pub fn add(&mut self, op: BatchEditOp) {
        self.edits.push(op);
    }

    /// Apply all edits atomically
    ///
    /// Returns error if any edit fails validation. Edits are applied bottom-up
    /// within each file to preserve line numbers.
    pub fn apply(&self, root: &Path) -> Result<BatchEditResult, String> {
        if self.edits.is_empty() {
            return Ok(BatchEditResult {
                files_modified: Vec::new(),
                edits_applied: 0,
                errors: Vec::new(),
            });
        }

        // Phase 1: Resolve all targets and group by file
        let mut by_file: HashMap<PathBuf, Vec<(usize, &BatchEditOp, SymbolLocation)>> =
            HashMap::new();
        let mut errors = Vec::new();
        let editor = Editor::new();

        for (idx, op) in self.edits.iter().enumerate() {
            match self.resolve_target(root, &op.target, &editor) {
                Ok((file_path, location)) => {
                    by_file
                        .entry(file_path)
                        .or_default()
                        .push((idx, op, location));
                }
                Err(e) => {
                    errors.push((op.target.clone(), e));
                }
            }
        }

        // If any target failed to resolve, abort
        if !errors.is_empty() {
            return Err(format!(
                "Failed to resolve {} target(s): {}",
                errors.len(),
                errors
                    .iter()
                    .map(|(t, e)| format!("{}: {}", t, e))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }

        // Phase 2: Check for overlapping edits within files
        for (path, file_edits) in &by_file {
            self.check_overlaps(path, file_edits)?;
        }

        // Phase 3: Apply edits in memory (bottom-up to preserve line numbers)
        // Collect all modified contents before writing anything - true atomicity
        let mut modified_contents: Vec<(PathBuf, String)> = Vec::new();
        let mut edits_applied = 0;

        for (path, mut file_edits) in by_file {
            // Sort by start line descending (apply bottom-up)
            file_edits.sort_by(|a, b| b.2.start_line.cmp(&a.2.start_line));

            let mut content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

            for (_idx, op, loc) in &file_edits {
                content = self.apply_single_edit(&editor, &content, loc, &op.action)?;
                edits_applied += 1;
            }

            modified_contents.push((path, content));
        }

        // Phase 4: Write all files atomically (only if all edits succeeded)
        let mut files_modified = Vec::new();
        for (path, content) in modified_contents {
            std::fs::write(&path, &content)
                .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
            files_modified.push(path);
        }

        Ok(BatchEditResult {
            files_modified,
            edits_applied,
            errors,
        })
    }

    /// Preview all edits without applying them
    ///
    /// Returns the original and modified content for each file so callers can
    /// display a diff. Does not modify any files.
    pub fn preview(&self, root: &Path) -> Result<BatchPreviewResult, String> {
        if self.edits.is_empty() {
            return Ok(BatchPreviewResult {
                files: Vec::new(),
                total_edits: 0,
            });
        }

        // Phase 1: Resolve all targets and group by file
        let mut by_file: HashMap<PathBuf, Vec<(usize, &BatchEditOp, SymbolLocation)>> =
            HashMap::new();
        let mut errors = Vec::new();
        let editor = Editor::new();

        for (idx, op) in self.edits.iter().enumerate() {
            match self.resolve_target(root, &op.target, &editor) {
                Ok((file_path, location)) => {
                    by_file
                        .entry(file_path)
                        .or_default()
                        .push((idx, op, location));
                }
                Err(e) => {
                    errors.push((op.target.clone(), e));
                }
            }
        }

        if !errors.is_empty() {
            return Err(format!(
                "Failed to resolve {} target(s): {}",
                errors.len(),
                errors
                    .iter()
                    .map(|(t, e)| format!("{}: {}", t, e))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }

        // Phase 2: Check for overlapping edits
        for (path, file_edits) in &by_file {
            self.check_overlaps(path, file_edits)?;
        }

        // Phase 3: Compute modified content for each file
        let mut file_previews = Vec::new();
        let mut total_edits = 0;

        for (path, mut file_edits) in by_file {
            file_edits.sort_by(|a, b| b.2.start_line.cmp(&a.2.start_line));

            let original = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

            let mut modified = original.clone();
            let edit_count = file_edits.len();

            for (_idx, op, loc) in &file_edits {
                modified = self.apply_single_edit(&editor, &modified, loc, &op.action)?;
            }

            total_edits += edit_count;
            file_previews.push(FilePreview {
                path,
                original,
                modified,
                edit_count,
            });
        }

        Ok(BatchPreviewResult {
            files: file_previews,
            total_edits,
        })
    }

    /// Resolve a target string to file path and symbol location
    fn resolve_target(
        &self,
        root: &Path,
        target: &str,
        editor: &Editor,
    ) -> Result<(PathBuf, SymbolLocation), String> {
        // Use unified path resolution
        let unified = path_resolve::resolve_unified(target, root)
            .ok_or_else(|| format!("Could not resolve path: {}", target))?;

        let file_path = root.join(&unified.file_path);
        if !file_path.exists() {
            return Err(format!("File not found: {}", file_path.display()));
        }

        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        // Get symbol name from path
        let symbol_name = unified
            .symbol_path
            .last()
            .ok_or_else(|| format!("No symbol specified in target: {}", target))?;

        let location = editor
            .find_symbol(&file_path, &content, symbol_name, false)
            .ok_or_else(|| format!("Symbol not found: {}", symbol_name))?;

        Ok((file_path, location))
    }

    /// Check for overlapping edits in a file
    fn check_overlaps(
        &self,
        path: &Path,
        edits: &[(usize, &BatchEditOp, SymbolLocation)],
    ) -> Result<(), String> {
        for i in 0..edits.len() {
            for j in (i + 1)..edits.len() {
                let (_, op_a, loc_a) = &edits[i];
                let (_, op_b, loc_b) = &edits[j];

                // Check if ranges overlap
                let overlaps =
                    loc_a.start_line <= loc_b.end_line && loc_b.start_line <= loc_a.end_line;

                if overlaps {
                    return Err(format!(
                        "Overlapping edits in {}: {} (L{}-{}) and {} (L{}-{})",
                        path.display(),
                        op_a.target,
                        loc_a.start_line,
                        loc_a.end_line,
                        op_b.target,
                        loc_b.start_line,
                        loc_b.end_line
                    ));
                }
            }
        }
        Ok(())
    }

    /// Apply a single edit operation
    fn apply_single_edit(
        &self,
        editor: &Editor,
        content: &str,
        loc: &SymbolLocation,
        action: &BatchAction,
    ) -> Result<String, String> {
        match action {
            BatchAction::Delete => Ok(editor.delete_symbol(content, loc)),
            BatchAction::Replace { content: new } => Ok(editor.replace_symbol(content, loc, new)),
            BatchAction::Insert {
                content: new,
                position,
            } => match position.as_str() {
                "before" => Ok(editor.insert_before(content, loc, new)),
                "after" => Ok(editor.insert_after(content, loc, new)),
                _ => Err(format!("Invalid position: {} (use before/after)", position)),
            },
        }
    }
}

impl Default for BatchEdit {
    fn default() -> Self {
        Self::new()
    }
}
