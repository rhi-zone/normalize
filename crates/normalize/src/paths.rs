//! Path utilities for normalize data directories.
//!
//! Supports external index locations via NORMALIZE_INDEX_DIR environment variable.
//! This allows repos without `.normalize` in `.gitignore` to store indexes elsewhere.

use std::path::{Path, PathBuf};

/// Get the normalize data directory for a project.
///
/// Resolution order:
/// 1. If NORMALIZE_INDEX_DIR is set to an absolute path, use it directly
/// 2. If NORMALIZE_INDEX_DIR is set to a relative path, use $XDG_DATA_HOME/normalize/<relative>
/// 3. Otherwise, use <root>/.normalize
///
/// Examples:
/// - NORMALIZE_INDEX_DIR="/tmp/normalize-data" -> /tmp/normalize-data
/// - NORMALIZE_INDEX_DIR="myproject" -> ~/.local/share/normalize/myproject
/// - (unset) -> <root>/.normalize
pub fn get_normalize_dir(root: &Path) -> PathBuf {
    if let Ok(index_dir) = std::env::var("NORMALIZE_INDEX_DIR") {
        let path = PathBuf::from(&index_dir);
        if path.is_absolute() {
            return path;
        }
        // Relative path: use XDG_DATA_HOME/normalize/<relative>
        let data_home = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/share")
            });
        return data_home.join("normalize").join(&index_dir);
    }
    root.join(".normalize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // Mutex to serialize tests that modify environment variables.
    // set_var/remove_var are unsafe in edition 2024 due to potential data races
    // when other threads read the environment concurrently.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_normalize_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::remove_var("NORMALIZE_INDEX_DIR") };
        let root = PathBuf::from("/project");
        assert_eq!(
            get_normalize_dir(&root),
            PathBuf::from("/project/.normalize")
        );
    }

    #[test]
    fn test_absolute_normalize_index_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::set_var("NORMALIZE_INDEX_DIR", "/custom/path") };
        let root = PathBuf::from("/project");
        assert_eq!(get_normalize_dir(&root), PathBuf::from("/custom/path"));
        unsafe { env::remove_var("NORMALIZE_INDEX_DIR") };
    }

    #[test]
    fn test_relative_normalize_index_dir() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { env::set_var("NORMALIZE_INDEX_DIR", "myproject") };
        unsafe { env::set_var("XDG_DATA_HOME", "/home/user/.data") };
        let root = PathBuf::from("/project");
        assert_eq!(
            get_normalize_dir(&root),
            PathBuf::from("/home/user/.data/normalize/myproject")
        );
        unsafe { env::remove_var("NORMALIZE_INDEX_DIR") };
        unsafe { env::remove_var("XDG_DATA_HOME") };
    }
}
