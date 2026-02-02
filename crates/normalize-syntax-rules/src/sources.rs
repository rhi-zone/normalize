//! Rule source system for conditional rule evaluation.
//!
//! Sources provide data that rules can use in `requires` predicates:
//! ```toml
//! requires = { rust.edition = ">=2024" }
//! requires = { rust.is_test_file = "!" }  # exclude test files
//! requires = { env.CI = "true" }
//! requires = { path.matches = "**/tests/**" }
//! ```
//!
//! Built-in sources:
//! - `path` - file path matching (glob patterns)
//! - `env` - environment variables
//! - `git` - repository state (branch, staged, dirty)
//! - `config` - .normalize/config.toml values
//! - Language sources: `rust`, `typescript`, `python`, `go`, etc.

use std::collections::HashMap;
use std::path::Path;

/// Parse a simple TOML value from ` = "value"` or ` = 'value'`.
/// Used for quick line-based parsing of config files.
fn parse_toml_value(rest: &str) -> Option<String> {
    let rest = rest.trim();
    let rest = rest.strip_prefix('=')?;
    let rest = rest.trim();

    // Handle quoted strings
    if let Some(rest) = rest.strip_prefix('"') {
        return rest.strip_suffix('"').map(|s| s.to_string());
    }
    if let Some(rest) = rest.strip_prefix('\'') {
        return rest.strip_suffix('\'').map(|s| s.to_string());
    }

    // Handle unquoted values (numbers, etc.)
    Some(rest.to_string())
}

/// Context passed to sources for evaluation.
pub struct SourceContext<'a> {
    /// Absolute path to the file being analyzed.
    pub file_path: &'a Path,
    /// Path relative to project root.
    pub rel_path: &'a str,
    /// Project root directory.
    pub project_root: &'a Path,
}

/// A source of data for rule conditionals.
///
/// Each source owns a namespace (e.g., "rust", "env", "path") and provides
/// key-value data that rules can query in `requires` predicates.
pub trait RuleSource: Send + Sync {
    /// The namespace this source provides (e.g., "rust", "env", "path").
    fn namespace(&self) -> &str;

    /// Evaluate the source for a given file context.
    ///
    /// Returns a map of key-value pairs available under this namespace.
    /// For example, RustSource might return `{"edition": "2024", "resolver": "2"}`.
    ///
    /// Returns `None` if this source doesn't apply to the given file
    /// (e.g., RustSource returns None for Python files).
    fn evaluate(&self, ctx: &SourceContext) -> Option<HashMap<String, String>>;
}

/// Registry of all available rule sources.
#[derive(Default)]
pub struct SourceRegistry {
    sources: Vec<Box<dyn RuleSource>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a source. Sources are evaluated in registration order.
    pub fn register(&mut self, source: Box<dyn RuleSource>) {
        self.sources.push(source);
    }

    /// Get a specific value by full key (e.g., "rust.edition").
    pub fn get(&self, ctx: &SourceContext, key: &str) -> Option<String> {
        // Parse namespace.key
        let (ns, field) = key.split_once('.')?;

        for source in &self.sources {
            if source.namespace() == ns
                && let Some(values) = source.evaluate(ctx)
            {
                return values.get(field).cloned();
            }
        }
        None
    }
}

// ============================================================================
// Built-in sources
// ============================================================================

/// Environment variable source.
///
/// Provides `env.VAR_NAME` for any environment variable.
pub struct EnvSource;

impl RuleSource for EnvSource {
    fn namespace(&self) -> &str {
        "env"
    }

    fn evaluate(&self, _ctx: &SourceContext) -> Option<HashMap<String, String>> {
        // Return all env vars - could be optimized to lazy evaluation
        Some(std::env::vars().collect())
    }
}

/// Path-based source for glob matching.
///
/// Provides `path.matches` for checking if file matches a pattern.
/// Note: This is evaluated specially since it needs the pattern from requires.
pub struct PathSource;

impl RuleSource for PathSource {
    fn namespace(&self) -> &str {
        "path"
    }

    fn evaluate(&self, ctx: &SourceContext) -> Option<HashMap<String, String>> {
        let mut result = HashMap::new();
        result.insert("rel".to_string(), ctx.rel_path.to_string());
        result.insert(
            "abs".to_string(),
            ctx.file_path.to_string_lossy().to_string(),
        );
        if let Some(ext) = ctx.file_path.extension() {
            result.insert("ext".to_string(), ext.to_string_lossy().to_string());
        }
        if let Some(name) = ctx.file_path.file_name() {
            result.insert("filename".to_string(), name.to_string_lossy().to_string());
        }
        Some(result)
    }
}

/// Git repository state source.
///
/// Provides `git.branch`, `git.dirty`, `git.staged`.
pub struct GitSource;

impl RuleSource for GitSource {
    fn namespace(&self) -> &str {
        "git"
    }

    fn evaluate(&self, ctx: &SourceContext) -> Option<HashMap<String, String>> {
        let mut result = HashMap::new();

        // Get current branch
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(ctx.project_root)
            .output()
            && output.status.success()
        {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            result.insert("branch".to_string(), branch);
        }

        // Check if file is staged
        if let Ok(output) = std::process::Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(ctx.project_root)
            .output()
            && output.status.success()
        {
            let staged = String::from_utf8_lossy(&output.stdout);
            let is_staged = staged.lines().any(|l| l == ctx.rel_path);
            result.insert("staged".to_string(), is_staged.to_string());
        }

        // Check if repo is dirty
        if let Ok(output) = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(ctx.project_root)
            .output()
            && output.status.success()
        {
            let dirty = !output.stdout.is_empty();
            result.insert("dirty".to_string(), dirty.to_string());
        }

        Some(result)
    }
}

/// Rust project source - parses Cargo.toml for edition, resolver, etc.
///
/// Provides:
/// - `rust.edition`, `rust.resolver`, `rust.name`, `rust.version` - from Cargo.toml
/// - `rust.is_test_file` - true if file is in tests/, named *_test.rs, or has top-level #[cfg(test)]
pub struct RustSource;

impl RustSource {
    /// Find the nearest Cargo.toml for a given file path.
    fn find_cargo_toml(file_path: &Path) -> Option<std::path::PathBuf> {
        let mut current = file_path.parent()?;
        loop {
            let cargo_toml = current.join("Cargo.toml");
            if cargo_toml.exists() {
                return Some(cargo_toml);
            }
            current = current.parent()?;
        }
    }

    /// Find the workspace root Cargo.toml (the one with [workspace] section).
    fn find_workspace_root(start: &Path) -> Option<std::path::PathBuf> {
        let mut current = start.parent()?;
        loop {
            let cargo_toml = current.join("Cargo.toml");
            if cargo_toml.exists()
                && let Ok(content) = std::fs::read_to_string(&cargo_toml)
                && let Ok(parsed) = content.parse::<toml::Table>()
                && parsed.contains_key("workspace")
            {
                return Some(cargo_toml);
            }
            current = current.parent()?;
        }
    }

    /// Parse Cargo.toml, resolving workspace inheritance.
    fn parse_cargo_toml(cargo_toml_path: &Path) -> HashMap<String, String> {
        let mut result = HashMap::new();

        let content = match std::fs::read_to_string(cargo_toml_path) {
            Ok(c) => c,
            Err(_) => return result,
        };

        let parsed: toml::Table = match content.parse() {
            Ok(t) => t,
            Err(_) => return result,
        };

        // Get package table
        let package = match parsed.get("package").and_then(|v| v.as_table()) {
            Some(p) => p,
            None => return result,
        };

        // Keys we're interested in
        let keys = ["edition", "resolver", "name", "version"];

        // Try to get workspace values lazily (only if needed)
        let mut workspace_package: Option<&toml::Table> = None;
        let mut workspace_parsed: Option<toml::Table> = None;

        for key in keys {
            if let Some(value) = package.get(key) {
                // Check for workspace inheritance: { workspace = true }
                if let Some(table) = value.as_table()
                    && table.get("workspace").and_then(|v| v.as_bool()) == Some(true)
                {
                    // Lazily load workspace Cargo.toml
                    if workspace_package.is_none() {
                        if let Some(ws_path) = Self::find_workspace_root(cargo_toml_path)
                            && let Ok(ws_content) = std::fs::read_to_string(&ws_path)
                            && let Ok(ws_parsed) = ws_content.parse::<toml::Table>()
                        {
                            workspace_parsed = Some(ws_parsed);
                        }
                        workspace_package = workspace_parsed
                            .as_ref()
                            .and_then(|ws| ws.get("workspace"))
                            .and_then(|w| w.as_table())
                            .and_then(|w| w.get("package"))
                            .and_then(|p| p.as_table());
                    }

                    // Get value from workspace
                    if let Some(ws_pkg) = workspace_package
                        && let Some(ws_value) = ws_pkg.get(key)
                    {
                        if let Some(s) = ws_value.as_str() {
                            result.insert(key.to_string(), s.to_string());
                        } else if let Some(i) = ws_value.as_integer() {
                            result.insert(key.to_string(), i.to_string());
                        }
                    }
                    continue;
                }

                // Direct value
                if let Some(s) = value.as_str() {
                    result.insert(key.to_string(), s.to_string());
                } else if let Some(i) = value.as_integer() {
                    result.insert(key.to_string(), i.to_string());
                }
            }
        }

        result
    }

    /// Detect if a file is a test file based on path patterns and content.
    fn is_test_file(ctx: &SourceContext) -> bool {
        let path = ctx.rel_path;

        // Path-based detection
        if path.starts_with("tests/")
            || path.starts_with("tests\\")
            || path.contains("/tests/")
            || path.contains("\\tests\\")
        {
            return true;
        }

        // Filename patterns
        if let Some(filename) = ctx.file_path.file_name().and_then(|n| n.to_str())
            && (filename.ends_with("_test.rs")
                || filename.ends_with("_tests.rs")
                || filename.starts_with("test_"))
        {
            return true;
        }

        // Check file content for top-level #[cfg(test)]
        // This catches files that are primarily test code
        if let Ok(content) = std::fs::read_to_string(ctx.file_path) {
            // Look for #[cfg(test)] at start of line (top-level attribute)
            for line in content.lines().take(50) {
                // Skip to first non-comment, non-empty line with #[cfg(test)]
                let trimmed = line.trim();
                if trimmed.starts_with("#[cfg(test)]") {
                    return true;
                }
            }
        }

        false
    }
}

impl RuleSource for RustSource {
    fn namespace(&self) -> &str {
        "rust"
    }

    fn evaluate(&self, ctx: &SourceContext) -> Option<HashMap<String, String>> {
        // Only apply to Rust files
        let ext = ctx.file_path.extension()?;
        if ext != "rs" {
            return None;
        }

        // Find nearest Cargo.toml
        let cargo_toml = Self::find_cargo_toml(ctx.file_path);

        let mut result = cargo_toml
            .map(|p| Self::parse_cargo_toml(&p))
            .unwrap_or_default();

        // Detect test files
        result.insert(
            "is_test_file".to_string(),
            Self::is_test_file(ctx).to_string(),
        );

        Some(result)
    }
}

/// TypeScript/JavaScript project source - parses tsconfig.json and package.json.
///
/// Provides `typescript.target`, `typescript.module`, `typescript.strict`, `node.version`.
pub struct TypeScriptSource;

impl TypeScriptSource {
    /// Find the nearest tsconfig.json for a given file path.
    fn find_tsconfig(file_path: &Path) -> Option<std::path::PathBuf> {
        let mut current = file_path.parent()?;
        loop {
            let tsconfig = current.join("tsconfig.json");
            if tsconfig.exists() {
                return Some(tsconfig);
            }
            current = current.parent()?;
        }
    }

    /// Find the nearest package.json for a given file path.
    fn find_package_json(file_path: &Path) -> Option<std::path::PathBuf> {
        let mut current = file_path.parent()?;
        loop {
            let pkg = current.join("package.json");
            if pkg.exists() {
                return Some(pkg);
            }
            current = current.parent()?;
        }
    }

    /// Parse tsconfig.json for compilerOptions.
    fn parse_tsconfig(content: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // Simple JSON parsing for key fields in compilerOptions
        // Look for "target", "module", "strict", "moduleResolution"
        for line in content.lines() {
            let line = line.trim();

            if let Some(value) = Self::extract_json_string(line, "target") {
                result.insert("target".to_string(), value);
            } else if let Some(value) = Self::extract_json_string(line, "module") {
                result.insert("module".to_string(), value);
            } else if let Some(value) = Self::extract_json_string(line, "moduleResolution") {
                result.insert("moduleResolution".to_string(), value);
            } else if line.contains("\"strict\"") {
                if line.contains("true") {
                    result.insert("strict".to_string(), "true".to_string());
                } else if line.contains("false") {
                    result.insert("strict".to_string(), "false".to_string());
                }
            }
        }

        result
    }

    /// Parse package.json for engines.node.
    fn parse_package_json(content: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // Look for engines.node version
        let mut in_engines = false;
        for line in content.lines() {
            let line = line.trim();
            if line.contains("\"engines\"") {
                in_engines = true;
            } else if in_engines {
                if line.starts_with('}') {
                    in_engines = false;
                } else if let Some(value) = Self::extract_json_string(line, "node") {
                    result.insert("node_version".to_string(), value);
                }
            }

            // Also get name and version
            if let Some(value) = Self::extract_json_string(line, "name")
                && !result.contains_key("name")
            {
                result.insert("name".to_string(), value);
            }
            if let Some(value) = Self::extract_json_string(line, "version")
                && !result.contains_key("version")
            {
                result.insert("version".to_string(), value);
            }
        }

        result
    }

    /// Extract a JSON string value: `"key": "value"` or `"key": "value",`
    fn extract_json_string(line: &str, key: &str) -> Option<String> {
        let pattern = format!("\"{}\"", key);
        if !line.contains(&pattern) {
            return None;
        }

        // Find the value after the colon
        let colon_pos = line.find(':')?;
        let after_colon = line[colon_pos + 1..].trim();

        // Extract quoted string
        if let Some(rest) = after_colon.strip_prefix('"') {
            let end = rest.find('"')?;
            return Some(rest[..end].to_string());
        }

        None
    }
}

impl RuleSource for TypeScriptSource {
    fn namespace(&self) -> &str {
        "typescript"
    }

    fn evaluate(&self, ctx: &SourceContext) -> Option<HashMap<String, String>> {
        // Only apply to TypeScript/JavaScript files
        let ext = ctx.file_path.extension()?.to_string_lossy();
        if !matches!(ext.as_ref(), "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs") {
            return None;
        }

        let mut result = HashMap::new();

        // Parse tsconfig.json if present
        if let Some(tsconfig) = Self::find_tsconfig(ctx.file_path)
            && let Ok(content) = std::fs::read_to_string(&tsconfig)
        {
            result.extend(Self::parse_tsconfig(&content));
        }

        // Parse package.json if present
        if let Some(pkg_json) = Self::find_package_json(ctx.file_path)
            && let Ok(content) = std::fs::read_to_string(&pkg_json)
        {
            result.extend(Self::parse_package_json(&content));
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

/// Python project source - parses pyproject.toml for project metadata.
///
/// Provides `python.version`, `python.name`.
pub struct PythonSource;

impl PythonSource {
    /// Find the nearest pyproject.toml for a given file path.
    fn find_pyproject(file_path: &Path) -> Option<std::path::PathBuf> {
        let mut current = file_path.parent()?;
        loop {
            let pyproject = current.join("pyproject.toml");
            if pyproject.exists() {
                return Some(pyproject);
            }
            current = current.parent()?;
        }
    }

    /// Parse pyproject.toml for project metadata.
    fn parse_pyproject(content: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // Simple TOML parsing for key fields
        // Look for requires-python, name, version
        for line in content.lines() {
            let line = line.trim();

            if let Some(rest) = line.strip_prefix("requires-python")
                && let Some(value) = parse_toml_value(rest)
            {
                // Strip comparison operators for the version
                let version = value
                    .trim_start_matches(">=")
                    .trim_start_matches("<=")
                    .trim_start_matches("==")
                    .trim_start_matches('^')
                    .trim_start_matches('~');
                result.insert("requires_python".to_string(), version.to_string());
            } else if let Some(rest) = line.strip_prefix("name")
                && let Some(value) = parse_toml_value(rest)
            {
                result.insert("name".to_string(), value);
            } else if let Some(rest) = line.strip_prefix("version")
                && let Some(value) = parse_toml_value(rest)
            {
                result.insert("version".to_string(), value);
            }
        }

        result
    }
}

impl RuleSource for PythonSource {
    fn namespace(&self) -> &str {
        "python"
    }

    fn evaluate(&self, ctx: &SourceContext) -> Option<HashMap<String, String>> {
        // Only apply to Python files
        let ext = ctx.file_path.extension()?;
        if ext != "py" {
            return None;
        }

        // Find nearest pyproject.toml
        let pyproject = Self::find_pyproject(ctx.file_path)?;
        let content = std::fs::read_to_string(&pyproject).ok()?;

        let result = Self::parse_pyproject(&content);
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

/// Go project source - parses go.mod for module metadata.
///
/// Provides `go.version`, `go.module`.
pub struct GoSource;

impl GoSource {
    /// Find the nearest go.mod for a given file path.
    fn find_go_mod(file_path: &Path) -> Option<std::path::PathBuf> {
        let mut current = file_path.parent()?;
        loop {
            let go_mod = current.join("go.mod");
            if go_mod.exists() {
                return Some(go_mod);
            }
            current = current.parent()?;
        }
    }

    /// Parse go.mod for module metadata.
    fn parse_go_mod(content: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();

        for line in content.lines() {
            let line = line.trim();

            // module github.com/user/repo
            if let Some(rest) = line.strip_prefix("module ") {
                result.insert("module".to_string(), rest.trim().to_string());
            }
            // go 1.21
            else if let Some(rest) = line.strip_prefix("go ") {
                result.insert("version".to_string(), rest.trim().to_string());
            }
        }

        result
    }
}

impl RuleSource for GoSource {
    fn namespace(&self) -> &str {
        "go"
    }

    fn evaluate(&self, ctx: &SourceContext) -> Option<HashMap<String, String>> {
        // Only apply to Go files
        let ext = ctx.file_path.extension()?;
        if ext != "go" {
            return None;
        }

        // Find nearest go.mod
        let go_mod = Self::find_go_mod(ctx.file_path)?;
        let content = std::fs::read_to_string(&go_mod).ok()?;

        let result = Self::parse_go_mod(&content);
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}

/// Create a registry with all built-in sources.
pub fn builtin_registry() -> SourceRegistry {
    let mut registry = SourceRegistry::new();
    registry.register(Box::new(EnvSource));
    registry.register(Box::new(PathSource));
    registry.register(Box::new(GitSource));
    registry.register(Box::new(RustSource));
    registry.register(Box::new(TypeScriptSource));
    registry.register(Box::new(PythonSource));
    registry.register(Box::new(GoSource));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_source() {
        // SAFETY: Test runs single-threaded, no concurrent env access
        unsafe {
            std::env::set_var("MOSS_TEST_VAR", "hello");
        }

        let ctx = SourceContext {
            file_path: Path::new("/tmp/test.rs"),
            rel_path: "test.rs",
            project_root: Path::new("/tmp"),
        };

        let registry = builtin_registry();
        let value = registry.get(&ctx, "env.MOSS_TEST_VAR");
        assert_eq!(value, Some("hello".to_string()));

        // SAFETY: Test cleanup
        unsafe {
            std::env::remove_var("MOSS_TEST_VAR");
        }
    }

    #[test]
    fn test_path_source() {
        let ctx = SourceContext {
            file_path: Path::new("/project/src/lib.rs"),
            rel_path: "src/lib.rs",
            project_root: Path::new("/project"),
        };

        let registry = builtin_registry();
        assert_eq!(
            registry.get(&ctx, "path.rel"),
            Some("src/lib.rs".to_string())
        );
        assert_eq!(registry.get(&ctx, "path.ext"), Some("rs".to_string()));
        assert_eq!(
            registry.get(&ctx, "path.filename"),
            Some("lib.rs".to_string())
        );
    }

    #[test]
    fn test_rust_source_parse_cargo_toml() {
        let temp_dir = std::env::temp_dir().join("moss_test_cargo_toml");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let cargo_path = temp_dir.join("Cargo.toml");
        let content = r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2024"
resolver = "2"
"#;
        std::fs::write(&cargo_path, content).unwrap();
        let result = RustSource::parse_cargo_toml(&cargo_path);
        assert_eq!(result.get("name"), Some(&"my-crate".to_string()));
        assert_eq!(result.get("version"), Some(&"0.1.0".to_string()));
        assert_eq!(result.get("edition"), Some(&"2024".to_string()));
        assert_eq!(result.get("resolver"), Some(&"2".to_string()));
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_rust_source_real_file() {
        // Test against this project's actual Cargo.toml
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let file_path = manifest_dir.join("src/lib.rs");
        let ctx = SourceContext {
            file_path: &file_path,
            rel_path: "src/lib.rs",
            project_root: manifest_dir,
        };

        let registry = builtin_registry();
        // Should find edition from Cargo.toml
        let edition = registry.get(&ctx, "rust.edition");
        assert!(edition.is_some(), "Should find rust.edition");
    }

    #[test]
    fn test_typescript_source_parse_tsconfig() {
        let content = r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "strict": true,
    "moduleResolution": "bundler"
  }
}"#;
        let result = TypeScriptSource::parse_tsconfig(content);
        assert_eq!(result.get("target"), Some(&"ES2020".to_string()));
        assert_eq!(result.get("module"), Some(&"ESNext".to_string()));
        assert_eq!(result.get("strict"), Some(&"true".to_string()));
        assert_eq!(result.get("moduleResolution"), Some(&"bundler".to_string()));
    }

    #[test]
    fn test_typescript_source_parse_package_json() {
        let content = r#"{
  "name": "my-app",
  "version": "1.0.0",
  "engines": {
    "node": ">=18.0.0"
  }
}"#;
        let result = TypeScriptSource::parse_package_json(content);
        assert_eq!(result.get("name"), Some(&"my-app".to_string()));
        assert_eq!(result.get("version"), Some(&"1.0.0".to_string()));
        assert_eq!(result.get("node_version"), Some(&">=18.0.0".to_string()));
    }

    #[test]
    fn test_python_source_parse_pyproject() {
        let content = r#"
[project]
name = "my-package"
version = "0.1.0"
requires-python = ">=3.10"
"#;
        let result = PythonSource::parse_pyproject(content);
        assert_eq!(result.get("name"), Some(&"my-package".to_string()));
        assert_eq!(result.get("version"), Some(&"0.1.0".to_string()));
        assert_eq!(result.get("requires_python"), Some(&"3.10".to_string()));
    }

    #[test]
    fn test_go_source_parse_go_mod() {
        let content = r#"module github.com/user/repo

go 1.21

require (
    golang.org/x/text v0.3.0
)"#;
        let result = GoSource::parse_go_mod(content);
        assert_eq!(
            result.get("module"),
            Some(&"github.com/user/repo".to_string())
        );
        assert_eq!(result.get("version"), Some(&"1.21".to_string()));
    }

    #[test]
    fn test_rust_is_test_file() {
        // Path-based detection: /tests/ directory
        let ctx = SourceContext {
            file_path: Path::new("/project/tests/integration.rs"),
            rel_path: "tests/integration.rs",
            project_root: Path::new("/project"),
        };
        assert!(RustSource::is_test_file(&ctx));

        // Filename pattern: *_test.rs
        let ctx = SourceContext {
            file_path: Path::new("/project/src/foo_test.rs"),
            rel_path: "src/foo_test.rs",
            project_root: Path::new("/project"),
        };
        assert!(RustSource::is_test_file(&ctx));

        // Filename pattern: test_*.rs
        let ctx = SourceContext {
            file_path: Path::new("/project/src/test_bar.rs"),
            rel_path: "src/test_bar.rs",
            project_root: Path::new("/project"),
        };
        assert!(RustSource::is_test_file(&ctx));

        // Not a test file
        let ctx = SourceContext {
            file_path: Path::new("/project/src/lib.rs"),
            rel_path: "src/lib.rs",
            project_root: Path::new("/project"),
        };
        assert!(!RustSource::is_test_file(&ctx));
    }
}
