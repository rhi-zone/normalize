//! Project dependency management for multiple package ecosystems.
//!
//! Provides the [`Ecosystem`] trait for detecting and querying package
//! ecosystems (cargo, npm, pip, etc.) in project directories.
//!
//! # Example
//!
//! ```ignore
//! use normalize_ecosystems::{detect_ecosystem, PackageInfo};
//! use std::path::Path;
//!
//! if let Some(ecosystem) = detect_ecosystem(Path::new(".")) {
//!     if let Ok(info) = ecosystem.query("serde", Path::new(".")) {
//!         println!("{}: {}", info.name, info.version);
//!     }
//! }
//! ```

mod cache;
pub mod doc_tree;
pub mod docs_rs;
pub mod ecosystems;
pub mod http;
pub mod local_docs;
pub mod source_archive;
pub mod symbol_docs;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ============================================================================
// Types
// ============================================================================

/// Parsed package query (name with optional version).
#[derive(Debug, Clone)]
pub struct PackageQuery {
    /// Package name to look up (e.g. "serde", "express").
    pub name: String,
    /// Optional version constraint (e.g. "1.0", "^2.3"); `None` means latest.
    pub version: Option<String>,
}

impl PackageQuery {
    /// Parse "package" or "package@version" format.
    pub fn parse(input: &str) -> Self {
        if let Some((name, version)) = input.rsplit_once('@') {
            PackageQuery {
                name: name.to_string(),
                version: Some(version.to_string()),
            }
        } else {
            PackageQuery {
                name: input.to_string(),
                version: None,
            }
        }
    }

    /// Cache key: "package@version" or "package@latest"
    pub fn cache_key(&self) -> String {
        match &self.version {
            Some(v) => format!("{}@{}", self.name, v),
            None => format!("{}@latest", self.name),
        }
    }
}

/// Information about a package from a registry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PackageInfo {
    /// The package name as registered (e.g. "serde", "express").
    pub name: String,
    /// The resolved version string (e.g. "1.0.152").
    pub version: String,
    /// Short human-readable description of the package, if available.
    pub description: Option<String>,
    /// SPDX license identifier or expression, if available (e.g. "MIT", "Apache-2.0").
    pub license: Option<String>,
    /// URL of the package homepage or documentation site.
    pub homepage: Option<String>,
    /// URL of the source code repository.
    pub repository: Option<String>,
    /// Optional feature flags (Rust features, Python extras, npm optional deps).
    pub features: Vec<Feature>,
    /// Direct dependencies declared by this package.
    pub dependencies: Vec<Dependency>,
}

/// A package feature (Rust features, Python extras, npm optional deps).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Feature {
    /// Feature name (e.g. "derive", "full", "async").
    pub name: String,
    /// Optional description of what this feature enables.
    pub description: Option<String>,
    /// Other features or packages this feature depends on.
    pub dependencies: Vec<String>,
}

/// Source of a dependency (registry, git, local path).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DepSource {
    #[default]
    Registry,
    Git {
        url: String,
    },
    Path {
        path: String,
    },
}

/// A package dependency.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Dependency {
    pub name: String,
    /// Actual package name if renamed (e.g. cargo `package = "..."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
    pub version_req: Option<String>,
    pub optional: bool,
    #[serde(default)]
    pub source: DepSource,
}

impl Dependency {
    /// Create a registry dependency (the common case).
    pub fn registry(name: impl Into<String>, version_req: Option<String>, optional: bool) -> Self {
        Self {
            name: name.into(),
            package_name: None,
            version_req,
            optional,
            source: DepSource::Registry,
        }
    }

    /// Create a git dependency.
    pub fn git(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            package_name: None,
            version_req: None,
            optional: false,
            source: DepSource::Git { url: url.into() },
        }
    }

    /// Create a path dependency.
    pub fn path(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            package_name: None,
            version_req: None,
            optional: false,
            source: DepSource::Path { path: path.into() },
        }
    }

    /// The effective package name (package_name if set, otherwise name).
    pub fn effective_name(&self) -> &str {
        self.package_name.as_deref().unwrap_or(&self.name)
    }
}

/// A node in the dependency tree.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TreeNode {
    /// Package name.
    pub name: String,
    /// Resolved version string (may be empty if unknown).
    pub version: String,
    /// Transitive dependencies of this node.
    pub dependencies: Vec<TreeNode>,
}

/// Full dependency tree rooted at the direct dependencies of the project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DependencyTree {
    /// Top-level packages (direct dependencies of the project).
    pub roots: Vec<TreeNode>,
}

/// Security vulnerability found by audit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Vulnerability {
    /// The affected package name.
    pub package: String,
    /// The affected version string.
    pub version: String,
    /// Severity level of this vulnerability.
    pub severity: VulnerabilitySeverity,
    /// Short human-readable description of the vulnerability.
    pub title: String,
    /// URL to the advisory or vulnerability database entry.
    pub url: Option<String>,
    /// CVE identifier (e.g. "CVE-2024-12345"), if assigned.
    pub cve: Option<String>,
    /// Version in which the vulnerability was fixed, if known.
    pub fixed_in: Option<String>,
}

/// Severity level for vulnerabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VulnerabilitySeverity {
    Critical,
    High,
    Medium,
    Low,
    Unknown,
}

impl VulnerabilitySeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            VulnerabilitySeverity::Critical => "critical",
            VulnerabilitySeverity::High => "high",
            VulnerabilitySeverity::Medium => "medium",
            VulnerabilitySeverity::Low => "low",
            VulnerabilitySeverity::Unknown => "unknown",
        }
    }
}

/// Result of security audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    pub vulnerabilities: Vec<Vulnerability>,
}

/// Error type for package operations.
#[derive(Debug)]
pub enum PackageError {
    /// No tool found in PATH for this ecosystem
    NoToolFound,
    /// Tool execution failed
    ToolFailed(String),
    /// Failed to parse tool output
    ParseError(String),
    /// Package not found in registry
    NotFound(String),
    /// Network or registry error
    RegistryError(String),
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageError::NoToolFound => write!(f, "no package manager found in PATH"),
            PackageError::ToolFailed(msg) => write!(f, "tool failed: {}", msg),
            PackageError::ParseError(msg) => write!(f, "parse error: {}", msg),
            PackageError::NotFound(name) => write!(f, "package not found: {}", name),
            PackageError::RegistryError(msg) => write!(f, "registry error: {}", msg),
        }
    }
}

impl std::error::Error for PackageError {}

// ============================================================================
// Docs extraction traits and coordinator
// ============================================================================

/// Error type for documentation extraction and fetching.
#[derive(Debug)]
pub enum DocsError {
    /// The requested symbol or package was not found.
    NotFound(String),
    /// A local tool (e.g. `cargo metadata`) failed to run or returned an error.
    ToolFailed(String),
    /// Parsing of source or remote response failed.
    ParseError(String),
    /// A network error occurred while fetching remote docs.
    NetworkError(String),
}

impl std::fmt::Display for DocsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocsError::NotFound(msg) => write!(f, "not found: {}", msg),
            DocsError::ToolFailed(msg) => write!(f, "tool failed: {}", msg),
            DocsError::ParseError(msg) => write!(f, "parse error: {}", msg),
            DocsError::NetworkError(msg) => write!(f, "network error: {}", msg),
        }
    }
}

impl std::error::Error for DocsError {}

/// Convert a [`PackageError`] into a [`DocsError`] (used by the remote fetcher adapter).
impl From<PackageError> for DocsError {
    fn from(e: PackageError) -> Self {
        match e {
            PackageError::NotFound(msg) => DocsError::NotFound(msg),
            PackageError::ToolFailed(msg) => DocsError::ToolFailed(msg),
            PackageError::ParseError(msg) => DocsError::ParseError(msg),
            PackageError::RegistryError(msg) => DocsError::NetworkError(msg),
            PackageError::NoToolFound => DocsError::ToolFailed("no tool found".to_string()),
        }
    }
}

/// Per-language/ecosystem extractor that reads doc comments from on-disk source.
///
/// Implementations resolve the package to its on-disk source directory (via the
/// package manager's metadata command), then parse doc comments from the source.
///
/// # Implementing for a new ecosystem
///
/// 1. Implement this trait for your language (e.g. `NpmLocalDocsExtractor`).
/// 2. In `extract_docs`, resolve `package` to the on-disk source via the
///    ecosystem's metadata tool (e.g. `npm list --json`, `go list`, etc.).
/// 3. Walk the module tree to find `symbol_path` and extract doc comments.
pub trait LocalDocsExtractor: Send + Sync {
    /// Extract documentation for `symbol_path` from on-disk source.
    ///
    /// `package` is the package/crate name (e.g. `"serde"`).
    /// `symbol_path` is the full dotted path (e.g. `"serde::Serialize"`).
    /// `version` is the exact version; `None` means "whatever is locally present".
    fn extract_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<symbol_docs::SymbolDoc, DocsError>;
}

/// Per-ecosystem fetcher that retrieves docs from a remote registry / docs site.
///
/// Used as the fallback when [`LocalDocsExtractor`] fails (e.g. the package is
/// not installed locally, or no Cargo.lock is present).
///
/// # Implementing for a new ecosystem
///
/// Implement `fetch_docs` to hit the appropriate remote docs source (docs.rs,
/// pkg.go.dev, PyPI, npm.runkit.com, etc.) and return a populated [`SymbolDoc`].
pub trait RemoteDocsFetcher: Send + Sync {
    /// Fetch documentation for `symbol_path` from a remote source.
    fn fetch_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<symbol_docs::SymbolDoc, DocsError>;
}

/// Coordinator: try local extraction first, fall back to remote on any error.
///
/// This is the single entry-point that the CLI and KG cache layer call.
/// The caller is responsible for cache lookup *before* this and cache write
/// *after* (the coordinator itself is cache-unaware).
pub fn fetch_symbol_docs_with_fallback(
    local: &dyn LocalDocsExtractor,
    remote: &dyn RemoteDocsFetcher,
    package: &str,
    symbol_path: &str,
    version: Option<&str>,
) -> Result<symbol_docs::SymbolDoc, DocsError> {
    match local.extract_docs(package, symbol_path, version) {
        Ok(doc) => Ok(doc),
        Err(local_err) => {
            // Local failed — try remote
            remote
                .fetch_docs(package, symbol_path, version)
                .map_err(|remote_err| {
                    // Both failed: surface the remote error (usually more informative)
                    // but annotate with the local reason
                    DocsError::NotFound(format!("local: {}; remote: {}", local_err, remote_err))
                })
        }
    }
}

// ============================================================================
// Ecosystem trait
// ============================================================================

/// A lockfile pattern and its associated package manager.
pub struct LockfileManager {
    /// Lockfile filename to match (e.g. "package-lock.json", "Cargo.lock").
    pub filename: &'static str,
    /// Package manager that produced this lockfile (e.g. "npm", "cargo").
    pub manager: &'static str,
}

/// Trait for package ecosystem implementations.
///
/// Each ecosystem (cargo, npm, pip, etc.) implements this trait to provide:
/// - Detection via manifest files and lockfiles
/// - Package info queries via available tools
pub trait Ecosystem: Send + Sync {
    /// Display name for this ecosystem (e.g., "cargo", "npm", "pip")
    fn name(&self) -> &'static str;

    /// Manifest files that indicate this ecosystem (e.g., ["Cargo.toml"])
    fn manifest_files(&self) -> &'static [&'static str];

    /// Lockfiles and their associated package managers.
    /// Used to detect which specific tool to prefer.
    fn lockfiles(&self) -> &'static [LockfileManager];

    /// Available tools for this ecosystem, fastest first.
    /// Detection will try each until one is found in PATH.
    fn tools(&self) -> &'static [&'static str];

    /// Fetch package info using the specified tool.
    /// If version is None, fetches latest.
    fn fetch_info(&self, query: &PackageQuery, tool: &str) -> Result<PackageInfo, PackageError>;

    /// Look up installed version from lockfile.
    /// Returns None if no lockfile or package not found.
    fn installed_version(&self, package: &str, project_root: &Path) -> Option<String>;

    /// List declared dependencies from manifest file.
    fn list_dependencies(&self, project_root: &Path) -> Result<Vec<Dependency>, PackageError>;

    /// Get dependency tree from lockfile.
    /// Returns structured tree data.
    fn dependency_tree(&self, project_root: &Path) -> Result<DependencyTree, PackageError>;

    /// Package names this project publishes (from manifest, no network calls).
    /// Default: empty vec.
    fn published_names(&self, _project_root: &Path) -> Vec<String> {
        Vec::new()
    }

    /// Run security audit for known vulnerabilities.
    /// Default implementation returns empty result (no audit tool available).
    fn audit(&self, project_root: &Path) -> Result<AuditResult, PackageError>;

    /// Local documentation extractor for this ecosystem, if any.
    ///
    /// Returns a [`LocalDocsExtractor`] that reads doc comments from on-disk
    /// source (resolved via the ecosystem's metadata tool). `None` means the
    /// ecosystem has no local extraction path.
    fn docs_extractor(&self, _project_root: &Path) -> Option<Box<dyn LocalDocsExtractor>> {
        None
    }

    /// Remote documentation fetcher for this ecosystem, if any.
    ///
    /// Returns a [`RemoteDocsFetcher`] that retrieves docs from a remote
    /// registry / docs site (docs.rs, pkg.go.dev, etc.). `None` means the
    /// ecosystem has no remote fetch path.
    fn docs_fetcher(&self) -> Option<Box<dyn RemoteDocsFetcher>> {
        None
    }

    /// Split a doc query symbol into `(package, symbol_path)`.
    ///
    /// `symbol` is the user-supplied symbol (version already stripped), e.g.
    /// `"serde::Serialize"`. Returns the package/crate name and the full symbol
    /// path. `None` means the ecosystem cannot interpret this symbol syntax.
    fn package_from_symbol(&self, _symbol: &str) -> Option<(String, String)> {
        None
    }

    /// Source-code language for docs produced by this ecosystem (e.g. "rust").
    ///
    /// Used to construct the knowledge-graph cache ID before a doc is fetched.
    /// Defaults to [`Ecosystem::name`]; override when the ecosystem name and the
    /// language differ (e.g. "cargo" → "rust").
    fn docs_language(&self) -> &'static str {
        self.name()
    }

    /// Find the first available tool in PATH.
    fn find_tool(&self) -> Option<&'static str> {
        self.tools().iter().copied().find(|tool| which(tool))
    }

    /// Detect preferred tool from lockfiles, falling back to first available.
    fn detect_tool(&self, project_root: &Path) -> Option<&'static str> {
        // Check lockfiles first
        for lock in self.lockfiles() {
            if project_root.join(lock.filename).exists() && which(lock.manager) {
                return Some(lock.manager);
            }
        }
        // Fall back to first available tool
        self.find_tool()
    }

    /// Convenience method: detect tool and fetch info with caching.
    ///
    /// Accepts "package" or "package@version" format.
    /// If no version specified, checks lockfile for installed version first.
    /// Strategy: try cache first if fresh, else network, cache on success, stale cache as fallback.
    fn query(&self, package: &str, project_root: &Path) -> Result<PackageInfo, PackageError> {
        use std::time::Duration;

        let mut query = PackageQuery::parse(package);

        // If no explicit version, check lockfile for installed version
        if query.version.is_none() {
            query.version = self.installed_version(&query.name, project_root);
        }

        let tool = self
            .detect_tool(project_root)
            .ok_or(PackageError::NoToolFound)?;
        let cache_key = query.cache_key();
        let cache_ttl = Duration::from_secs(24 * 60 * 60); // 24 hours

        // Check fresh cache first (avoid network if recently cached)
        if let Some(cached) = cache::read(self.name(), &cache_key, cache_ttl) {
            return Ok(cached);
        }

        // Try network
        match self.fetch_info(&query, tool) {
            Ok(info) => {
                cache::write(self.name(), &cache_key, &info);
                Ok(info)
            }
            Err(e) => {
                // Network failed - try stale cache
                if let Some(cached) = cache::read_any(self.name(), &cache_key) {
                    return Ok(cached);
                }
                Err(e)
            }
        }
    }
}

/// Check if a command exists in PATH.
fn which(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let path = dir.join(cmd);
                path.is_file() && is_executable(&path)
            })
        })
        .unwrap_or(false)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

// Re-export ecosystem detection functions
pub use ecosystems::{
    all_ecosystems, detect_all_ecosystems, detect_ecosystem, get_ecosystem, list_ecosystems,
    register as register_ecosystem,
};

// Re-export SymbolDoc for convenience
pub use symbol_docs::{DocFormat, SymbolDoc};

// Re-export docs traits and coordinator
pub use docs_rs::DocsRsFetcher;
pub use local_docs::CargoLocalDocsExtractor;
