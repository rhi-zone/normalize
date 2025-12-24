//! Index management commands.

use clap::Subcommand;
use std::path::Path;

use crate::commands::{index_packages, index_stats, list_files, reindex};

#[derive(Subcommand)]
pub enum IndexAction {
    /// Rebuild the file index
    Rebuild {
        /// Also rebuild the call graph (slower, parses all files)
        #[arg(short, long)]
        call_graph: bool,
    },

    /// Show index statistics (DB size vs codebase size)
    Stats,

    /// List indexed files (with optional prefix filter)
    Files {
        /// Filter files by prefix
        prefix: Option<String>,

        /// Maximum number of files to show
        #[arg(short, long, default_value = "100")]
        limit: usize,
    },

    /// Index external packages (stdlib, site-packages) into global cache
    Packages {
        /// Ecosystems to index (python, go, js, deno, java, cpp, rust). Defaults to all available.
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,

        /// Clear existing index before re-indexing
        #[arg(long)]
        clear: bool,
    },
}

/// Run an index management action
pub fn cmd_index(action: IndexAction, root: Option<&Path>, json: bool) -> i32 {
    match action {
        IndexAction::Rebuild { call_graph } => reindex::cmd_reindex(root, call_graph),
        IndexAction::Stats => index_stats::cmd_index_stats(root, json),
        IndexAction::Files { prefix, limit } => {
            list_files::cmd_list_files(prefix.as_deref(), root, limit, json)
        }
        IndexAction::Packages { only, clear } => {
            index_packages::cmd_index_packages(&only, clear, root, json)
        }
    }
}
