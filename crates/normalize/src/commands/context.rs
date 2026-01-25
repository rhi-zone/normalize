//! Directory context: hierarchical context files.
//!
//! Collects and merges `.context.md` and `CONTEXT.md` files from the directory
//! hierarchy, from project root to target path.

use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};

use crate::output::OutputFormat;

/// Context file names to look for (in priority order).
const CONTEXT_FILES: &[&str] = &[".context.md", "CONTEXT.md"];

#[derive(Args, Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ContextArgs {
    /// Target path to collect context for
    #[arg(default_value = ".")]
    #[serde(default = "default_target")]
    pub target: String,

    /// Root directory (defaults to current directory)
    #[arg(short, long)]
    pub root: Option<PathBuf>,

    /// Show only file paths, not content
    #[arg(long)]
    #[serde(default)]
    pub list: bool,
}

/// Helper for serde default target
fn default_target() -> String {
    ".".to_string()
}

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(ContextArgs);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Run context command.
pub fn run(
    args: ContextArgs,
    format: OutputFormat,
    input_schema: bool,
    params_json: Option<&str>,
) -> i32 {
    if input_schema {
        print_input_schema();
        return 0;
    }
    // Override args with --params-json if provided
    let args = match params_json {
        Some(json) => match serde_json::from_str(json) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("error: invalid --params-json: {}", e);
                return 1;
            }
        },
        None => args,
    };
    let root = args
        .root
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let target = root.join(&args.target);

    // Determine the directory to collect context for
    let target_dir = if target.is_file() {
        target.parent().unwrap_or(&root).to_path_buf()
    } else {
        target.clone()
    };

    // Canonicalize paths for comparison
    let root = match root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to resolve root: {}", e);
            return 1;
        }
    };
    let target_dir = match target_dir.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to resolve target: {}", e);
            return 1;
        }
    };

    // Collect context files from root to target
    let files = collect_context_files(&root, &target_dir);

    if files.is_empty() {
        if format.is_json() {
            println!("[]");
        } else {
            println!("No context files found.");
        }
        return 0;
    }

    if args.list {
        if format.is_json() {
            let paths: Vec<&str> = files.iter().map(|f| f.to_str().unwrap_or("")).collect();
            println!("{}", serde_json::to_string_pretty(&paths).unwrap());
        } else {
            for file in &files {
                println!("{}", file.display());
            }
        }
        return 0;
    }

    // Read and output content
    if format.is_json() {
        let mut entries = Vec::new();
        for file in &files {
            let content = fs::read_to_string(file).unwrap_or_default();
            entries.push(serde_json::json!({
                "path": file.to_str().unwrap_or(""),
                "content": content,
            }));
        }
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
    } else {
        for (i, file) in files.iter().enumerate() {
            if i > 0 {
                println!();
            }
            // Show relative path from root
            let rel_path = file.strip_prefix(&root).unwrap_or(file);
            println!("# {}", rel_path.display());
            println!();
            match fs::read_to_string(file) {
                Ok(content) => print!("{}", content),
                Err(e) => eprintln!("Failed to read {}: {}", file.display(), e),
            }
        }
    }

    0
}

/// Collect context files from root to target directory.
/// Returns files in order from root to target (most general to most specific).
pub fn collect_context_files(root: &Path, target_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    // Build path from root to target
    let mut dirs = Vec::new();
    let mut current = target_dir.to_path_buf();

    // Walk up from target to root, collecting directories
    while current.starts_with(root) {
        dirs.push(current.clone());
        if current == root {
            break;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }

    // Reverse to get root-to-target order
    dirs.reverse();

    // Check each directory for context files
    for dir in dirs {
        for name in CONTEXT_FILES {
            let path = dir.join(name);
            if path.exists() {
                files.push(path);
                break; // Only take first match per directory
            }
        }
    }

    files
}

/// Get merged context content for a path.
/// Used by other commands (e.g., view --dir-context).
pub fn get_merged_context(root: &Path, target: &Path) -> Option<String> {
    // Find the target directory - walk up from target until we find an existing dir
    let target_dir = if target.is_file() {
        target.parent().unwrap_or(root).to_path_buf()
    } else if target.is_dir() {
        target.to_path_buf()
    } else {
        // Target doesn't exist - find first existing parent
        let mut dir = target.to_path_buf();
        while !dir.exists() {
            match dir.parent() {
                Some(p) => dir = p.to_path_buf(),
                None => return None,
            }
        }
        dir
    };

    let root = root.canonicalize().ok()?;
    let target_dir = target_dir.canonicalize().ok()?;

    let files = collect_context_files(&root, &target_dir);
    if files.is_empty() {
        return None;
    }

    let mut content = String::new();
    for (i, file) in files.iter().enumerate() {
        if i > 0 {
            content.push_str("\n\n");
        }
        if let Ok(text) = fs::read_to_string(file) {
            content.push_str(&text);
        }
    }

    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_collect_single_context_file() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();

        let files = collect_context_files(root, root);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("CONTEXT.md"));
    }

    #[test]
    fn test_collect_hierarchical_context() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Root context").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/.context.md"), "Src context").unwrap();

        let files = collect_context_files(root, &root.join("src"));
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("CONTEXT.md"));
        assert!(files[1].ends_with(".context.md"));
    }

    #[test]
    fn test_dotfile_takes_priority() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Uppercase").unwrap();
        fs::write(root.join(".context.md"), "Dotfile").unwrap();

        let files = collect_context_files(root, root);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".context.md"));
    }

    #[test]
    fn test_get_merged_context() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();

        fs::write(root.join("CONTEXT.md"), "Root").unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("sub/.context.md"), "Sub").unwrap();

        let content = get_merged_context(root, &root.join("sub/file.rs")).unwrap();
        assert!(content.contains("Root"));
        assert!(content.contains("Sub"));
    }

    #[test]
    fn test_no_context_files() {
        let tmp = tempdir().unwrap();
        let files = collect_context_files(tmp.path(), tmp.path());
        assert!(files.is_empty());
    }
}
