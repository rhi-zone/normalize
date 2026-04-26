//! Refactoring engine — composable semantic actions for code transformations.
//!
//! Three layers:
//! - **Actions** (`actions.rs`): Pure query and mutation primitives
//! - **Recipes** (`rename.rs`, future: `move.rs`, `extract.rs`): Compositions of actions
//! - **Executor** (`RefactoringExecutor`): Shared apply/dry-run/shadow logic

pub mod actions;
pub mod move_item;
pub mod rename;

use std::path::PathBuf;

use normalize_shadow::{EditInfo, Shadow};

/// A planned edit to a single file (not yet applied).
pub struct PlannedEdit {
    pub file: PathBuf,
    pub original: String,
    pub new_content: String,
    pub description: String,
}

/// A complete refactoring plan: multiple file edits + warnings.
pub struct RefactoringPlan {
    pub operation: String,
    pub edits: Vec<PlannedEdit>,
    pub warnings: Vec<String>,
}

/// Context available to all refactoring actions.
pub struct RefactoringContext {
    pub root: PathBuf,
    pub editor: normalize_edit::Editor,
    pub index: Option<normalize_facts::FileIndex>,
    pub loader: normalize_languages::GrammarLoader,
}

/// Cross-file references to a symbol.
pub struct References {
    pub callers: Vec<CallerRef>,
    pub importers: Vec<ImportRef>,
}

/// A call-site reference.
pub struct CallerRef {
    pub file: String,
    pub caller: String,
    pub line: usize,
    #[allow(dead_code)]
    pub access: Option<String>,
}

/// An import-site reference.
pub struct ImportRef {
    pub file: String,
    pub name: String,
    #[allow(dead_code)]
    pub alias: Option<String>,
    pub line: usize,
}

/// Executes a `RefactoringPlan`: writes files, manages shadow snapshots.
pub struct RefactoringExecutor {
    pub root: PathBuf,
    pub dry_run: bool,
    pub shadow_enabled: bool,
    pub message: Option<String>,
}

impl RefactoringExecutor {
    /// Apply the plan. On dry-run, returns the list of files that *would* change.
    /// On real run, writes files and records shadow history.
    pub fn apply(&self, plan: &RefactoringPlan) -> Result<Vec<String>, String> {
        if plan.edits.is_empty() {
            return Ok(vec![]);
        }

        let abs_paths: Vec<PathBuf> = plan.edits.iter().map(|e| e.file.clone()).collect();

        // Shadow: snapshot before
        if !self.dry_run && self.shadow_enabled {
            let shadow = Shadow::new(&self.root);
            if let Err(e) =
                shadow.before_edit(&abs_paths.iter().map(|p| p.as_path()).collect::<Vec<_>>())
            {
                eprintln!("warning: shadow git: {}", e);
            }
        }

        let mut modified: Vec<String> = vec![];

        for edit in &plan.edits {
            let rel_path = edit
                .file
                .strip_prefix(&self.root)
                .unwrap_or(&edit.file)
                .to_string_lossy()
                .to_string();

            if self.dry_run {
                if !modified.contains(&rel_path) {
                    modified.push(rel_path);
                }
            } else {
                match std::fs::write(&edit.file, &edit.new_content) {
                    Ok(_) => {
                        if !modified.contains(&rel_path) {
                            modified.push(rel_path);
                        }
                    }
                    Err(e) => eprintln!("error writing {}: {}", rel_path, e),
                }
            }
        }

        // Shadow: commit after
        if !self.dry_run && self.shadow_enabled && !modified.is_empty() {
            let shadow = Shadow::new(&self.root);
            let info = EditInfo {
                operation: plan.operation.clone(),
                target: plan
                    .edits
                    .first()
                    .map(|e| e.description.clone())
                    .unwrap_or_default(),
                files: abs_paths,
                message: self.message.clone(),
                workflow: None,
            };
            if let Err(e) = shadow.after_edit(&info) {
                eprintln!("warning: shadow git: {}", e);
            }
        }

        Ok(modified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_dry_run_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "original").unwrap();

        let executor = RefactoringExecutor {
            root: dir.path().to_path_buf(),
            dry_run: true,
            shadow_enabled: false,
            message: None,
        };

        let plan = RefactoringPlan {
            operation: "test".to_string(),
            edits: vec![PlannedEdit {
                file: file.clone(),
                original: "original".to_string(),
                new_content: "modified".to_string(),
                description: "test edit".to_string(),
            }],
            warnings: vec![],
        };

        let result = executor.apply(&plan).unwrap();
        assert_eq!(result, vec!["test.rs"]);
        // File unchanged
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");
    }

    #[test]
    fn executor_real_run_writes_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "original").unwrap();

        let executor = RefactoringExecutor {
            root: dir.path().to_path_buf(),
            dry_run: false,
            shadow_enabled: false,
            message: None,
        };

        let plan = RefactoringPlan {
            operation: "test".to_string(),
            edits: vec![PlannedEdit {
                file: file.clone(),
                original: "original".to_string(),
                new_content: "modified".to_string(),
                description: "test edit".to_string(),
            }],
            warnings: vec![],
        };

        let result = executor.apply(&plan).unwrap();
        assert_eq!(result, vec!["test.rs"]);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "modified");
    }

    #[test]
    fn executor_deduplicates_modified_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, "original").unwrap();

        let executor = RefactoringExecutor {
            root: dir.path().to_path_buf(),
            dry_run: false,
            shadow_enabled: false,
            message: None,
        };

        let plan = RefactoringPlan {
            operation: "test".to_string(),
            edits: vec![
                PlannedEdit {
                    file: file.clone(),
                    original: "original".to_string(),
                    new_content: "step1".to_string(),
                    description: "edit 1".to_string(),
                },
                PlannedEdit {
                    file: file.clone(),
                    original: "step1".to_string(),
                    new_content: "step2".to_string(),
                    description: "edit 2".to_string(),
                },
            ],
            warnings: vec![],
        };

        let result = executor.apply(&plan).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "step2");
    }

    #[test]
    fn empty_plan_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let executor = RefactoringExecutor {
            root: dir.path().to_path_buf(),
            dry_run: false,
            shadow_enabled: false,
            message: None,
        };

        let plan = RefactoringPlan {
            operation: "test".to_string(),
            edits: vec![],
            warnings: vec![],
        };

        let result = executor.apply(&plan).unwrap();
        assert!(result.is_empty());
    }
}
