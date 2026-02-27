//! Edit service for server-less CLI.

use crate::commands::edit::{EditAction, EditResult, Position};
use crate::service::history::HistoryService;
use server_less::cli;

/// Edit sub-service.
pub struct EditService {
    pub(crate) history: HistoryService,
}

#[cli(name = "edit", about = "Structural editing of code symbols")]
impl EditService {
    /// Delete a symbol
    #[allow(clippy::too_many_arguments)]
    pub fn delete(
        &self,
        #[param(positional, help = "Target to edit (path/Symbol)")] target: String,
        #[param(help = "Dry run - show what would change")] dry_run: bool,
        #[param(short = 'y', help = "Skip confirmation")] yes: bool,
        #[param(help = "Exclude files matching patterns")] exclude: Vec<String>,
        #[param(help = "Only include files matching patterns")] only: Vec<String>,
        #[param(help = "Allow glob patterns to match multiple symbols")] multiple: bool,
        #[param(short = 'm', help = "Message for shadow history")] message: Option<String>,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        crate::commands::edit::cmd_edit_service(
            &target,
            EditAction::Delete,
            root.as_deref(),
            dry_run,
            yes,
            &exclude,
            &only,
            multiple,
            message.as_deref(),
            case_insensitive,
        )
    }

    /// Replace a symbol with new content
    #[allow(clippy::too_many_arguments)]
    pub fn replace(
        &self,
        #[param(positional, help = "Target to edit (path/Symbol)")] target: String,
        #[param(positional, help = "Replacement content")] content: String,
        #[param(help = "Dry run - show what would change")] dry_run: bool,
        #[param(help = "Exclude files matching patterns")] exclude: Vec<String>,
        #[param(help = "Only include files matching patterns")] only: Vec<String>,
        #[param(help = "Allow glob patterns to match multiple symbols")] multiple: bool,
        #[param(short = 'm', help = "Message for shadow history")] message: Option<String>,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        crate::commands::edit::cmd_edit_service(
            &target,
            EditAction::Replace { content },
            root.as_deref(),
            dry_run,
            false,
            &exclude,
            &only,
            multiple,
            message.as_deref(),
            case_insensitive,
        )
    }

    /// Swap two symbols
    #[allow(clippy::too_many_arguments)]
    pub fn swap(
        &self,
        #[param(positional, help = "Target to edit (path/Symbol)")] target: String,
        #[param(positional, help = "Symbol to swap with")] other: String,
        #[param(help = "Dry run - show what would change")] dry_run: bool,
        #[param(help = "Exclude files matching patterns")] exclude: Vec<String>,
        #[param(help = "Only include files matching patterns")] only: Vec<String>,
        #[param(short = 'm', help = "Message for shadow history")] message: Option<String>,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        crate::commands::edit::cmd_edit_service(
            &target,
            EditAction::Swap { other },
            root.as_deref(),
            dry_run,
            false,
            &exclude,
            &only,
            false,
            message.as_deref(),
            case_insensitive,
        )
    }

    /// Insert content relative to a symbol
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &self,
        #[param(positional, help = "Target symbol")] target: String,
        #[param(positional, help = "Content to insert")] content: String,
        #[param(help = "Position: before, after, prepend, append")] at: String,
        #[param(help = "Dry run - show what would change")] dry_run: bool,
        #[param(help = "Exclude files matching patterns")] exclude: Vec<String>,
        #[param(help = "Only include files matching patterns")] only: Vec<String>,
        #[param(short = 'm', help = "Message for shadow history")] message: Option<String>,
        #[param(short = 'i', help = "Case-insensitive matching")] case_insensitive: bool,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        let position: Position = at.parse()?;
        crate::commands::edit::cmd_edit_service(
            &target,
            EditAction::Insert {
                content,
                at: position,
            },
            root.as_deref(),
            dry_run,
            false,
            &exclude,
            &only,
            false,
            message.as_deref(),
            case_insensitive,
        )
    }

    /// Undo the last N edits
    pub fn undo(
        &self,
        #[param(positional, help = "Number of edits to undo (default: 1)")] count: Option<usize>,
        #[param(help = "Undo changes only for specific file")] file: Option<String>,
        #[param(help = "Allow undo to cross git commit boundaries")] cross_checkpoint: bool,
        #[param(help = "Dry run - show what would change")] dry_run: bool,
        #[param(help = "Force undo even if files were modified externally")] force: bool,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        crate::commands::edit::cmd_undo_redo_service(
            root.as_deref(),
            Some(count.unwrap_or(1)),
            false,
            None,
            file.as_deref(),
            cross_checkpoint,
            dry_run,
            force,
        )
    }

    /// Redo the last undone edit
    pub fn redo(
        &self,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        crate::commands::edit::cmd_undo_redo_service(
            root.as_deref(),
            None,
            true,
            None,
            None,
            false,
            false,
            false,
        )
    }

    /// Jump to a specific shadow commit
    pub fn goto(
        &self,
        #[param(positional, help = "Shadow commit reference")] commit_ref: String,
        #[param(help = "Dry run - show what would change")] dry_run: bool,
        #[param(help = "Force even if files were modified externally")] force: bool,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        crate::commands::edit::cmd_undo_redo_service(
            root.as_deref(),
            None,
            false,
            Some(&commit_ref),
            None,
            false,
            dry_run,
            force,
        )
    }

    /// Apply batch edits from JSON file
    pub fn batch(
        &self,
        #[param(positional, help = "JSON file with edits (or - for stdin)")] file: String,
        #[param(help = "Dry run - show what would change")] dry_run: bool,
        #[param(short = 'm', help = "Message for shadow history")] message: Option<String>,
        #[param(short = 'r', help = "Root directory")] root: Option<String>,
    ) -> Result<EditResult, String> {
        crate::commands::edit::cmd_batch_edit_service(
            &file,
            root.as_deref(),
            dry_run,
            message.as_deref(),
        )
    }

    /// View shadow git edit history
    pub fn history(&self) -> &HistoryService {
        &self.history
    }
}
