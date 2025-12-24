//! Go module parsing for import resolution.
//!
//! Parses go.mod files to understand module paths and resolve imports.

use std::path::Path;

/// Information from a go.mod file
#[derive(Debug, Clone)]
pub struct GoModule {
    /// Module path (e.g., "github.com/user/project")
    pub path: String,
    /// Go version (e.g., "1.21")
    pub go_version: Option<String>,
}

/// Parse a go.mod file to extract module information.
pub fn parse_go_mod(path: &Path) -> Option<GoModule> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_go_mod_content(&content)
}

/// Parse go.mod content string.
pub fn parse_go_mod_content(content: &str) -> Option<GoModule> {
    let mut module_path = None;
    let mut go_version = None;

    for line in content.lines() {
        let line = line.trim();

        // module github.com/user/project
        if line.starts_with("module ") {
            module_path = Some(line.trim_start_matches("module ").trim().to_string());
        }

        // go 1.21
        if line.starts_with("go ") {
            go_version = Some(line.trim_start_matches("go ").trim().to_string());
        }
    }

    module_path.map(|path| GoModule { path, go_version })
}

/// Find go.mod by walking up from a directory.
pub fn find_go_mod(start: &Path) -> Option<std::path::PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let go_mod = current.join("go.mod");
        if go_mod.exists() {
            return Some(go_mod);
        }

        if !current.pop() {
            break;
        }
    }

    None
}

/// Resolve a Go import path to a local directory path.
///
/// Returns the computed path if the import is within the module, None for external imports.
/// Does not check if the path exists - caller should verify.
pub fn resolve_go_import(import_path: &str, module: &GoModule, project_root: &Path) -> Option<std::path::PathBuf> {
    // Check if import is within our module
    if !import_path.starts_with(&module.path) {
        return None; // External import
    }

    // Get the relative path after the module prefix
    let rel_path = import_path.strip_prefix(&module.path)?;
    let rel_path = rel_path.trim_start_matches('/');

    let target = if rel_path.is_empty() {
        project_root.to_path_buf()
    } else {
        project_root.join(rel_path)
    };

    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_go_mod() {
        let content = r#"
module github.com/user/project

go 1.21

require (
    github.com/pkg/errors v0.9.1
    golang.org/x/sync v0.3.0
)
"#;
        let module = parse_go_mod_content(content).unwrap();
        assert_eq!(module.path, "github.com/user/project");
        assert_eq!(module.go_version, Some("1.21".to_string()));
    }

    #[test]
    fn test_resolve_internal_import() {
        let module = GoModule {
            path: "github.com/user/project".to_string(),
            go_version: Some("1.21".to_string()),
        };

        // Internal import
        let result = resolve_go_import(
            "github.com/user/project/pkg/utils",
            &module,
            Path::new("/fake/root"),
        );
        assert_eq!(result, Some(std::path::PathBuf::from("/fake/root/pkg/utils")));

        // External import
        let result = resolve_go_import("github.com/other/lib", &module, Path::new("/fake/root"));
        assert!(result.is_none());
    }
}
