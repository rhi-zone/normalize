//! File metadata types for code facts.

use serde::{Deserialize, Serialize};

/// Metadata about an indexed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    /// File path relative to the repo root.
    pub path: String,
    /// True if this entry is a directory rather than a file.
    pub is_dir: bool,
    /// Modification timestamp in seconds since Unix epoch.
    pub mtime: i64,
    /// Line count; 0 for directories.
    pub lines: usize,
}
