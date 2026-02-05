//! File metadata types for code facts.

use serde::{Deserialize, Serialize};

/// Metadata about an indexed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub path: String,
    pub is_dir: bool,
    pub mtime: i64,
    pub lines: usize,
}
