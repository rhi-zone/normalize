//! Package management service for server-less CLI.

use super::resolve_pretty;
use server_less::cli;
use std::cell::Cell;
use std::path::Path;

/// Package management sub-service.
pub struct PackageService {
    _pretty: Cell<bool>,
}

impl PackageService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            _pretty: Cell::new(pretty.get()),
        }
    }
}

/// Generic package command result.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct PackageResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for PackageResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref msg) = self.message {
            write!(f, "{}", msg)
        } else if self.success {
            write!(f, "Done")
        } else {
            write!(f, "Failed")
        }
    }
}

fn pretty_for(root: Option<&str>, pretty: bool, compact: bool) -> bool {
    let root = root.map(Path::new).unwrap_or(Path::new("."));
    resolve_pretty(root, pretty, compact)
}

#[cli(
    name = "package",
    about = "Package management: info, list, tree, outdated"
)]
impl PackageService {
    /// Query package info from registry
    pub fn info(
        &self,
        #[param(positional, help = "Package name to query (optionally with @version)")]
        package: String,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageResult, String> {
        crate::commands::package::cmd_package_service(
            "info",
            Some(&package),
            ecosystem.as_deref(),
            root.as_deref(),
            pretty_for(root.as_deref(), pretty, compact),
        )
    }

    /// List declared dependencies from manifest
    pub fn list(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageResult, String> {
        crate::commands::package::cmd_package_service(
            "list",
            None,
            ecosystem.as_deref(),
            root.as_deref(),
            pretty_for(root.as_deref(), pretty, compact),
        )
    }

    /// Show dependency tree from lockfile
    pub fn tree(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageResult, String> {
        crate::commands::package::cmd_package_service(
            "tree",
            None,
            ecosystem.as_deref(),
            root.as_deref(),
            pretty_for(root.as_deref(), pretty, compact),
        )
    }

    /// Show why a dependency is in the tree
    pub fn why(
        &self,
        #[param(positional, help = "Package name to trace")] package: String,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageResult, String> {
        crate::commands::package::cmd_package_service(
            "why",
            Some(&package),
            ecosystem.as_deref(),
            root.as_deref(),
            pretty_for(root.as_deref(), pretty, compact),
        )
    }

    /// Show outdated packages (installed vs latest)
    pub fn outdated(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageResult, String> {
        crate::commands::package::cmd_package_service(
            "outdated",
            None,
            ecosystem.as_deref(),
            root.as_deref(),
            pretty_for(root.as_deref(), pretty, compact),
        )
    }

    /// Check for security vulnerabilities
    pub fn audit(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageResult, String> {
        crate::commands::package::cmd_package_service(
            "audit",
            None,
            ecosystem.as_deref(),
            root.as_deref(),
            pretty_for(root.as_deref(), pretty, compact),
        )
    }
}
