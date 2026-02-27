//! Edit command for normalize CLI.

use std::path::Path;

use crate::config::NormalizeConfig;
use crate::edit::EditorExt;
use crate::shadow::{EditInfo, Shadow};
use crate::{daemon, edit, path_resolve};

struct EditOutput {
    file: String,
    symbol: Option<String>,
    operation: String,
    dry_run: bool,
    new_content: Option<String>,
}

struct UndoOutput {
    operation: String,
    dry_run: bool,
    changes: Vec<UndoChange>,
}

struct UndoChange {
    description: String,
    commit: String,
    files: Vec<String>,
    conflicts: Vec<String>,
}

struct BatchOutput {
    dry_run: bool,
    files_modified: Vec<String>,
}

/// Position for insert/move/copy operations
#[derive(Clone, Copy, clap::ValueEnum, serde::Deserialize, schemars::JsonSchema)]
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
#[derive(clap::Subcommand, serde::Deserialize, schemars::JsonSchema)]
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
        #[arg(long)]
        at: Position,
    },

    /// Move target to a new location
    Move {
        /// Destination symbol or container
        destination: String,
        /// Where to place: before, after, prepend, append
        #[arg(long)]
        at: Position,
    },

    /// Copy target to a new location
    Copy {
        /// Destination symbol or container
        destination: String,
        /// Where to place: before, after, prepend, append
        #[arg(long)]
        at: Position,
    },
}

/// Perform structural edits on a file.
#[allow(clippy::too_many_arguments)]
fn cmd_edit(
    target: &str,
    action: EditAction,
    root: Option<&Path>,
    dry_run: bool,
    yes: bool,
    exclude: &[String],
    only: &[String],
    multiple: bool,
    message: Option<&str>,
    case_insensitive: bool,
) -> Result<EditOutput, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let config = NormalizeConfig::load(&root);
    let shadow_enabled = config.shadow.enabled();

    if matches!(action, EditAction::Delete)
        && !yes
        && !dry_run
        && config.shadow.warn_on_delete.unwrap_or(true)
    {
        return Err(
            "Delete requires confirmation. Use --yes or -y to confirm.\n\
             To disable this warning: set [shadow] warn_on_delete = false in config"
                .to_string(),
        );
    }

    daemon::maybe_start_daemon(&root);

    let unified = path_resolve::resolve_unified(target, &root)
        .ok_or_else(|| format!("No matches for: {}", target))?;

    if unified.is_directory {
        return Err(format!("Cannot edit a directory: {}", target));
    }

    if super::build_filter(&root, exclude, only)
        .is_some_and(|f| !f.matches(Path::new(&unified.file_path)))
    {
        return Err(format!(
            "Target '{}' excluded by filter (resolved to {})",
            target, unified.file_path
        ));
    }

    let file_path = root.join(&unified.file_path);
    let content =
        std::fs::read_to_string(&file_path).map_err(|e| format!("Error reading file: {}", e))?;

    let editor = edit::Editor::new();

    if unified.symbol_path.is_empty() {
        return handle_file_level(
            &action,
            &editor,
            &content,
            &file_path,
            &unified.file_path,
            dry_run,
            &root,
            shadow_enabled,
            message,
        );
    }

    let symbol_pattern = unified.symbol_path.join("/");

    if edit::Editor::is_glob_pattern(&symbol_pattern) {
        return handle_glob_edit(
            &symbol_pattern,
            action,
            &editor,
            &content,
            &file_path,
            &unified.file_path,
            dry_run,
            multiple,
            &root,
            shadow_enabled,
            message,
            case_insensitive,
        );
    }

    let symbol_name = unified.symbol_path.last().unwrap();
    let loc = editor
        .find_symbol(&file_path, &content, symbol_name, case_insensitive)
        .ok_or_else(|| format!("Symbol not found: {}", symbol_name))?;

    let (operation, new_content) = match action {
        EditAction::Delete => (Operation::Delete, editor.delete_symbol(&content, &loc)),

        EditAction::Replace {
            content: ref new_code,
        } => (
            Operation::Replace,
            editor.replace_symbol(&content, &loc, new_code),
        ),

        EditAction::Swap { ref other } => {
            let other_loc = editor
                .find_symbol(&file_path, &content, other, case_insensitive)
                .ok_or_else(|| format!("Other symbol not found: {}", other))?;
            let (first_loc, second_loc) = if loc.start_byte < other_loc.start_byte {
                (&loc, &other_loc)
            } else {
                (&other_loc, &loc)
            };
            let first_content = content[first_loc.start_byte..first_loc.end_byte].to_string();
            let second_content = content[second_loc.start_byte..second_loc.end_byte].to_string();
            let mut new = content.clone();
            new.replace_range(second_loc.start_byte..second_loc.end_byte, &first_content);
            new.replace_range(first_loc.start_byte..first_loc.end_byte, &second_content);
            (Operation::Swap, new)
        }

        EditAction::Insert {
            content: ref insert_content,
            at,
        } => {
            let result = match at {
                Position::Before => editor.insert_before(&content, &loc, insert_content),
                Position::After => editor.insert_after(&content, &loc, insert_content),
                Position::Prepend | Position::Append => {
                    let body = editor
                        .find_container_body(&file_path, &content, symbol_name)
                        .ok_or_else(|| format!("Error: '{}' is not a container", symbol_name))?;
                    if matches!(at, Position::Prepend) {
                        editor.prepend_to_container(&content, &body, insert_content)
                    } else {
                        editor.append_to_container(&content, &body, insert_content)
                    }
                }
            };
            (Operation::Insert(at), result)
        }

        EditAction::Move {
            ref destination,
            at,
        } => {
            let source_content = content[loc.start_byte..loc.end_byte].to_string();
            let without_source = editor.delete_symbol(&content, &loc);
            let result = insert_single_at_destination(
                &editor,
                &file_path,
                &without_source,
                &source_content,
                destination,
                at,
                case_insensitive,
            )?;
            (Operation::Move(at), result)
        }

        EditAction::Copy {
            ref destination,
            at,
        } => {
            let source_content = &content[loc.start_byte..loc.end_byte];
            let result = insert_single_at_destination(
                &editor,
                &file_path,
                &content,
                source_content,
                destination,
                at,
                case_insensitive,
            )?;
            (Operation::Copy(at), result)
        }
    };

    apply_edit(
        dry_run,
        &unified.file_path,
        Some(symbol_name),
        &operation.to_string(),
        &new_content,
        &file_path,
        &root,
        shadow_enabled,
        message,
    )
}

/// Handle file-level operations (prepend/append to file without symbol target).
#[allow(clippy::too_many_arguments)]
fn handle_file_level(
    action: &EditAction,
    editor: &edit::Editor,
    content: &str,
    file_path: &Path,
    rel_path: &str,
    dry_run: bool,
    root: &Path,
    shadow_enabled: bool,
    message: Option<&str>,
) -> Result<EditOutput, String> {
    let (operation, new_content) = match action {
        EditAction::Insert {
            content: insert_content,
            at: Position::Prepend,
        } => (
            Operation::Insert(Position::Prepend),
            editor.prepend_to_file(content, insert_content),
        ),
        EditAction::Insert {
            content: insert_content,
            at: Position::Append,
        } => (
            Operation::Insert(Position::Append),
            editor.append_to_file(content, insert_content),
        ),
        _ => {
            return Err(
                "This operation requires a symbol target. Use a path like 'src/foo.py/MyClass'\n\
                 Hint: Only 'insert --at prepend' and 'insert --at append' work on files directly"
                    .to_string(),
            );
        }
    };

    apply_edit(
        dry_run,
        rel_path,
        None,
        &operation.to_string(),
        &new_content,
        file_path,
        root,
        shadow_enabled,
        message,
    )
}

/// Apply an edit (or return a dry-run preview) and return structured output.
/// Does not print anything — callers are responsible for display.
#[allow(clippy::too_many_arguments)]
fn apply_edit(
    dry_run: bool,
    rel_path: &str,
    symbol: Option<&str>,
    operation_name: &str,
    new_content: &str,
    file_path: &Path,
    root: &Path,
    shadow_enabled: bool,
    message: Option<&str>,
) -> Result<EditOutput, String> {
    if dry_run {
        return Ok(EditOutput {
            file: rel_path.to_string(),
            symbol: symbol.map(|s| s.to_string()),
            operation: operation_name.to_string(),
            dry_run: true,
            new_content: Some(new_content.to_string()),
        });
    }

    let shadow = if shadow_enabled {
        let s = Shadow::new(root);
        if let Err(e) = s.before_edit(&[file_path]) {
            eprintln!("warning: shadow git: {}", e);
        }
        Some(s)
    } else {
        None
    };

    std::fs::write(file_path, new_content).map_err(|e| format!("Error writing file: {}", e))?;

    if let Some(ref s) = shadow {
        let target = match symbol {
            Some(sym) => format!("{}/{}", rel_path, sym),
            None => rel_path.to_string(),
        };
        let info = EditInfo {
            operation: operation_name.to_string(),
            target,
            files: vec![file_path.to_path_buf()],
            message: message.map(String::from),
            workflow: None,
        };
        if let Err(e) = s.after_edit(&info) {
            eprintln!("warning: shadow git: {}", e);
        }
    }

    Ok(EditOutput {
        file: rel_path.to_string(),
        symbol: symbol.map(|s| s.to_string()),
        operation: operation_name.to_string(),
        dry_run: false,
        new_content: None,
    })
}

/// Insert content at a destination symbol or container.
/// Used by both Move and Copy operations to avoid duplication.
/// Returns Ok(new_content) on success, Err(error_message) on failure.
#[allow(clippy::too_many_arguments)]
fn insert_at_destination(
    editor: &edit::Editor,
    file_path: &Path,
    content: &str,
    original_content: &str,
    matches: &[edit::SymbolLocation],
    destination: &str,
    at: Position,
    case_insensitive: bool,
) -> Result<String, String> {
    let mut result = content.to_string();

    // Insert at destination, order depends on position type:
    // - append: original order [first..last] → iterate reversed matches
    // - others: reverse order [last..first] → iterate matches as-is
    let iter: Box<dyn Iterator<Item = _>> = if matches!(at, Position::Append) {
        Box::new(matches.iter().rev())
    } else {
        Box::new(matches.iter())
    };

    for loc in iter {
        let source_content = &original_content[loc.start_byte..loc.end_byte];
        result = match at {
            Position::Before | Position::After => {
                let dest_loc = editor
                    .find_symbol(file_path, &result, destination, case_insensitive)
                    .ok_or_else(|| format!("Destination not found: {}", destination))?;
                if matches!(at, Position::Before) {
                    editor.insert_before(&result, &dest_loc, source_content)
                } else {
                    editor.insert_after(&result, &dest_loc, source_content)
                }
            }
            Position::Prepend | Position::Append => {
                let body = editor
                    .find_container_body(file_path, &result, destination)
                    .ok_or_else(|| format!("Container not found: {}", destination))?;
                if matches!(at, Position::Prepend) {
                    editor.prepend_to_container(&result, &body, source_content)
                } else {
                    editor.append_to_container(&result, &body, source_content)
                }
            }
        };
    }

    Ok(result)
}

/// Insert source content at a destination symbol by position.
/// For single-symbol operations in cmd_edit.
fn insert_single_at_destination(
    editor: &edit::Editor,
    file_path: &std::path::Path,
    content: &str,
    source_content: &str,
    destination: &str,
    at: Position,
    case_insensitive: bool,
) -> Result<String, String> {
    match at {
        Position::Before | Position::After => {
            let dest_loc = editor
                .find_symbol(file_path, content, destination, case_insensitive)
                .ok_or_else(|| format!("Destination not found: {}", destination))?;
            Ok(if matches!(at, Position::Before) {
                editor.insert_before(content, &dest_loc, source_content)
            } else {
                editor.insert_after(content, &dest_loc, source_content)
            })
        }
        Position::Prepend | Position::Append => {
            let body = editor
                .find_container_body(file_path, content, destination)
                .ok_or_else(|| format!("Container not found: {}", destination))?;
            Ok(if matches!(at, Position::Prepend) {
                editor.prepend_to_container(content, &body, source_content)
            } else {
                editor.append_to_container(content, &body, source_content)
            })
        }
    }
}

/// Get the operation name for a position-based action.
fn position_op_name(prefix: &str, at: Position) -> &'static str {
    match (prefix, at) {
        ("move", Position::Before) => "move_before",
        ("move", Position::After) => "move_after",
        ("move", Position::Prepend) => "move_prepend",
        ("move", Position::Append) => "move_append",
        ("copy", Position::Before) => "copy_before",
        ("copy", Position::After) => "copy_after",
        ("copy", Position::Prepend) => "copy_prepend",
        ("copy", Position::Append) => "copy_append",
        ("insert", Position::Before) => "insert_before",
        ("insert", Position::After) => "insert_after",
        ("insert", Position::Prepend) => "prepend",
        ("insert", Position::Append) => "append",
        _ => "unknown",
    }
}

/// Handle glob pattern edits (multi-symbol operations)
#[allow(clippy::too_many_arguments)]
fn handle_glob_edit(
    pattern: &str,
    action: EditAction,
    editor: &edit::Editor,
    content: &str,
    file_path: &Path,
    rel_path: &str,
    dry_run: bool,
    multiple: bool,
    root: &Path,
    shadow_enabled: bool,
    message: Option<&str>,
    case_insensitive: bool,
) -> Result<EditOutput, String> {
    let matches = editor.find_symbols_matching(file_path, content, pattern);

    if matches.is_empty() {
        return Err(format!("No symbols match pattern: {}", pattern));
    }

    let count = matches.len();

    if count > 1 && !multiple {
        let names: Vec<&str> = matches.iter().map(|m| m.name.as_str()).collect();
        return Err(format!(
            "Pattern '{}' matches {} symbols. Use --multiple to confirm.\nMatched: {}",
            pattern,
            count,
            names.join(", ")
        ));
    }
    let names: Vec<String> = matches.iter().map(|m| m.name.clone()).collect();

    // Matches are sorted in reverse order (highest byte offset first) for safe deletion
    let (operation, new_content) = match action {
        EditAction::Delete => {
            let mut result = content.to_string();
            for loc in &matches {
                result = editor.delete_symbol(&result, loc);
            }
            ("delete", result)
        }

        EditAction::Replace {
            content: ref new_code,
        } => {
            let mut result = content.to_string();
            for loc in &matches {
                result = editor.replace_symbol(&result, loc, new_code);
            }
            ("replace", result)
        }

        EditAction::Insert {
            content: ref insert_content,
            at,
        } => {
            let mut result = content.to_string();
            for loc in &matches {
                result = match at {
                    Position::Before => editor.insert_before(&result, loc, insert_content),
                    Position::After => editor.insert_after(&result, loc, insert_content),
                    Position::Prepend | Position::Append => {
                        let body = editor
                            .find_container_body(file_path, &result, &loc.name)
                            .ok_or_else(|| format!("'{}' is not a container", loc.name))?;
                        if matches!(at, Position::Prepend) {
                            editor.prepend_to_container(&result, &body, insert_content)
                        } else {
                            editor.append_to_container(&result, &body, insert_content)
                        }
                    }
                };
            }
            (position_op_name("insert", at), result)
        }

        EditAction::Move {
            ref destination,
            at,
        } => {
            let mut result = content.to_string();
            for loc in &matches {
                result = editor.delete_symbol(&result, loc);
            }
            let new_content = insert_at_destination(
                editor,
                file_path,
                &result,
                content,
                &matches,
                destination,
                at,
                case_insensitive,
            )?;
            (position_op_name("move", at), new_content)
        }

        EditAction::Copy {
            ref destination,
            at,
        } => {
            let new_content = insert_at_destination(
                editor,
                file_path,
                content,
                content,
                &matches,
                destination,
                at,
                case_insensitive,
            )?;
            (position_op_name("copy", at), new_content)
        }

        EditAction::Swap { .. } => {
            return Err(format!(
                "'swap' is not supported with glob patterns (ambiguous pairing). Matched: {}",
                names.join(", ")
            ));
        }
    };

    apply_edit(
        dry_run,
        rel_path,
        Some(pattern),
        operation,
        &new_content,
        file_path,
        root,
        shadow_enabled,
        message,
    )
}

/// Handle undo/redo/goto operations on shadow git history.
#[allow(clippy::too_many_arguments)]
fn cmd_undo_redo(
    root: Option<&Path>,
    undo: Option<usize>,
    redo: bool,
    goto: Option<&str>,
    file_filter: Option<&str>,
    cross_checkpoint: bool,
    dry_run: bool,
    force: bool,
) -> Result<UndoOutput, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let shadow = Shadow::new(&root);

    if !shadow.exists() {
        return Err(
            "No shadow history exists. Make an edit first with `normalize edit`.".to_string(),
        );
    }

    if let Some(ref_str) = goto {
        let result = shadow
            .goto(ref_str, dry_run, force)
            .map_err(|e| e.to_string())?;
        return Ok(UndoOutput {
            operation: if dry_run {
                "goto_preview".to_string()
            } else {
                "goto".to_string()
            },
            dry_run,
            changes: vec![UndoChange {
                description: result.description,
                commit: result.undone_commit,
                files: result
                    .files
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
                conflicts: vec![],
            }],
        });
    }

    if redo {
        let result = shadow.redo().map_err(|e| e.to_string())?;
        return Ok(UndoOutput {
            operation: "redo".to_string(),
            dry_run: false,
            changes: vec![UndoChange {
                description: result.description,
                commit: result.undone_commit,
                files: result
                    .files
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
                conflicts: vec![],
            }],
        });
    }

    if let Some(count) = undo {
        let count = if count == 0 { 1 } else { count };
        let results = shadow
            .undo(count, file_filter, cross_checkpoint, dry_run, force)
            .map_err(|e| e.to_string())?;
        let changes = results
            .into_iter()
            .map(|r| UndoChange {
                description: r.description,
                commit: r.undone_commit,
                files: r.files.iter().map(|p| p.display().to_string()).collect(),
                conflicts: r.conflicts,
            })
            .collect();
        return Ok(UndoOutput {
            operation: if dry_run {
                "undo_preview".to_string()
            } else {
                "undo".to_string()
            },
            dry_run,
            changes,
        });
    }

    Err("No undo or redo operation specified".to_string())
}

/// Apply batch edits from a JSON file
fn cmd_batch_edit(
    batch_file: &str,
    root: Option<&Path>,
    dry_run: bool,
    message: Option<&str>,
) -> Result<BatchOutput, String> {
    let root = root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let json_content = if batch_file == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|_| "Failed to read from stdin".to_string())?;
        buf
    } else {
        std::fs::read_to_string(batch_file)
            .map_err(|e| format!("Failed to read {}: {}", batch_file, e))?
    };

    let batch = edit::BatchEdit::from_json(&json_content)
        .map_err(|e| format!("Failed to parse batch edits: {}", e))?;

    let batch = if let Some(msg) = message {
        batch.with_message(msg)
    } else {
        batch
    };

    if dry_run {
        let preview = batch
            .preview(&root)
            .map_err(|e| format!("Dry run failed: {}", e))?;
        let files_modified = preview
            .files
            .iter()
            .map(|f| f.path.display().to_string())
            .collect();
        return Ok(BatchOutput {
            dry_run: true,
            files_modified,
        });
    }

    let result = batch
        .apply(&root)
        .map_err(|e| format!("Batch edit failed: {}", e))?;

    let config = NormalizeConfig::load(&root);
    if config.shadow.enabled() {
        let shadow = Shadow::new(&root);
        if shadow.exists() {
            let file_refs: Vec<&Path> = result.files_modified.iter().map(|p| p.as_path()).collect();
            let _ = shadow.before_edit(&file_refs);
            let edit_info = EditInfo {
                operation: "batch".to_string(),
                target: format!("{} files", result.files_modified.len()),
                files: result.files_modified.clone(),
                message: message.map(|s| s.to_string()),
                workflow: None,
            };
            let _ = shadow.after_edit(&edit_info);
        }
    }

    Ok(BatchOutput {
        dry_run: false,
        files_modified: result
            .files_modified
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
    })
}

// ── Service-callable functions ────────────────────────────────────────

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
        if !self.files.is_empty() {
            // batch
            return write!(f, "Applied edits to {} file(s)", self.files.len());
        }
        // structural edit
        match (&self.symbol, &self.file) {
            (Some(sym), Some(file)) => write!(f, "{}: {} in {}", self.operation, sym, file),
            (None, Some(file)) => write!(f, "{}: {}", self.operation, file),
            _ => write!(f, "{} completed", self.operation),
        }
    }
}

/// Service-callable: perform a structural edit operation.
#[allow(clippy::too_many_arguments)]
pub fn cmd_edit_service(
    target: &str,
    action: EditAction,
    root: Option<&str>,
    dry_run: bool,
    yes: bool,
    exclude: &[String],
    only: &[String],
    multiple: bool,
    message: Option<&str>,
    case_insensitive: bool,
) -> Result<EditResult, String> {
    let root_path = root.map(Path::new);
    cmd_edit(
        target,
        action,
        root_path,
        dry_run,
        yes,
        exclude,
        only,
        multiple,
        message,
        case_insensitive,
    )
    .map(|out| EditResult {
        success: true,
        operation: out.operation,
        file: Some(out.file),
        symbol: out.symbol,
        dry_run: out.dry_run,
        new_content: out.new_content,
        changes: vec![],
        files: vec![],
    })
}

/// Service-callable: undo/redo/goto.
#[allow(clippy::too_many_arguments)]
pub fn cmd_undo_redo_service(
    root: Option<&str>,
    undo: Option<usize>,
    redo: bool,
    goto: Option<&str>,
    file_filter: Option<&str>,
    cross_checkpoint: bool,
    dry_run: bool,
    force: bool,
) -> Result<EditResult, String> {
    let root_path = root.map(Path::new);
    cmd_undo_redo(
        root_path,
        undo,
        redo,
        goto,
        file_filter,
        cross_checkpoint,
        dry_run,
        force,
    )
    .map(|out| EditResult {
        success: true,
        operation: out.operation,
        file: None,
        symbol: None,
        dry_run: out.dry_run,
        new_content: None,
        changes: out
            .changes
            .into_iter()
            .map(|c| EditChange {
                description: c.description,
                commit: c.commit,
                files: c.files,
                conflicts: c.conflicts,
            })
            .collect(),
        files: vec![],
    })
}

/// Service-callable: batch edit.
pub fn cmd_batch_edit_service(
    batch_file: &str,
    root: Option<&str>,
    dry_run: bool,
    message: Option<&str>,
) -> Result<EditResult, String> {
    let root_path = root.map(Path::new);
    cmd_batch_edit(batch_file, root_path, dry_run, message).map(|out| EditResult {
        success: true,
        operation: "batch".to_string(),
        file: None,
        symbol: None,
        dry_run: out.dry_run,
        new_content: None,
        changes: vec![],
        files: out.files_modified,
    })
}
