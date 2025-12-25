//! Package registry queries for multiple ecosystems.
//!
//! This crate provides a unified interface for querying package registries
//! across different language ecosystems (cargo, npm, pip, go, etc.).
//!
//! # Example
//!
//! ```ignore
//! use moss_packages::{detect_ecosystem, PackageInfo};
//! use std::path::Path;
//!
//! // Detect ecosystem from project files
//! if let Some(ecosystem) = detect_ecosystem(Path::new(".")) {
//!     // Query package info (with offline cache)
//!     if let Ok(info) = ecosystem.query("serde", Path::new(".")) {
//!         println!("{}: {}", info.name, info.version);
//!     }
//! }
//! ```

mod cache;
pub mod ecosystems;

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Information about a package from a registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub features: Vec<Feature>,
    pub dependencies: Vec<Dependency>,
}

/// A package feature (Rust features, Python extras, npm optional deps).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub description: Option<String>,
    pub dependencies: Vec<String>,
}

/// A package dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version_req: Option<String>,
    pub optional: bool,
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

/// A lockfile pattern and its associated package manager.
pub struct LockfileManager {
    pub filename: &'static str,
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
    fn fetch_info(&self, package: &str, tool: &str) -> Result<PackageInfo, PackageError>;

    /// Find the first available tool in PATH.
    fn find_tool(&self) -> Option<&'static str> {
        for tool in self.tools() {
            if which(tool) {
                return Some(tool);
            }
        }
        None
    }

    /// Detect preferred tool from lockfiles, falling back to first available.
    fn detect_tool(&self, project_root: &Path) -> Option<&'static str> {
        // Check lockfiles first
        for lock in self.lockfiles() {
            if project_root.join(lock.filename).exists() {
                if which(lock.manager) {
                    return Some(lock.manager);
                }
            }
        }
        // Fall back to first available tool
        self.find_tool()
    }

    /// Convenience method: detect tool and fetch info with caching.
    ///
    /// Strategy: try network first, cache on success, fall back to cache if network fails.
    /// Cache expires after 24 hours for fresh data, but stale cache is used for offline.
    fn query(&self, package: &str, project_root: &Path) -> Result<PackageInfo, PackageError> {
        use std::time::Duration;

        let tool = self.detect_tool(project_root).ok_or(PackageError::NoToolFound)?;
        let cache_ttl = Duration::from_secs(24 * 60 * 60); // 24 hours

        // Check fresh cache first (avoid network if recently cached)
        if let Some(cached) = cache::read(self.name(), package, cache_ttl) {
            return Ok(cached);
        }

        // Try network
        match self.fetch_info(package, tool) {
            Ok(info) => {
                cache::write(self.name(), package, &info);
                Ok(info)
            }
            Err(e) => {
                // Network failed - try stale cache
                if let Some(cached) = cache::read_any(self.name(), package) {
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

/// Detect ecosystem from project files in the given directory.
pub fn detect_ecosystem(project_root: &Path) -> Option<&'static dyn Ecosystem> {
    ecosystems::detect(project_root)
}

/// Get all registered ecosystems.
pub fn all_ecosystems() -> &'static [&'static dyn Ecosystem] {
    ecosystems::all()
}
