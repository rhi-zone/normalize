//! Package management service for server-less CLI.

use crate::commands::package::{PackageAction, run_package_action};
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

fn run_package(
    action: PackageAction,
    ecosystem: Option<&str>,
    root: Option<&str>,
) -> Result<PackageResult, String> {
    let root_path = root.map(Path::new);
    let exit_code = run_package_action(action, ecosystem, root_path);
    if exit_code == 0 {
        Ok(PackageResult {
            success: true,
            message: None,
            data: None,
        })
    } else {
        Err("Command failed".to_string())
    }
}

#[cli(
    name = "package",
    description = "Package management: info, list, tree, outdated"
)]
impl PackageService {
    /// Query package info from registry
    ///
    /// Examples:
    ///   normalize package info express                    # look up a package by name
    ///   normalize package info serde@1.0 -e cargo         # query a specific version and ecosystem
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
        let _ = (pretty, compact);
        run_package(
            PackageAction::Info { package },
            ecosystem.as_deref(),
            root.as_deref(),
        )
    }

    /// List declared dependencies from manifest
    ///
    /// Examples:
    ///   normalize package list                           # list dependencies for current project
    ///   normalize package list -e npm                    # list only npm dependencies
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
        let _ = (pretty, compact);
        run_package(PackageAction::List, ecosystem.as_deref(), root.as_deref())
    }

    /// Show dependency tree from lockfile
    ///
    /// Examples:
    ///   normalize package tree                           # show full dependency tree
    ///   normalize package tree -e cargo                  # show only Cargo dependency tree
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
        let _ = (pretty, compact);
        run_package(PackageAction::Tree, ecosystem.as_deref(), root.as_deref())
    }

    /// Show why a dependency is in the tree
    ///
    /// Examples:
    ///   normalize package why serde                      # trace why serde is a dependency
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
        let _ = (pretty, compact);
        run_package(
            PackageAction::Why { package },
            ecosystem.as_deref(),
            root.as_deref(),
        )
    }

    /// Show outdated packages (installed vs latest)
    ///
    /// Examples:
    ///   normalize package outdated                       # check all ecosystems for outdated deps
    ///   normalize package outdated -e npm                # check only npm packages
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
        let _ = (pretty, compact);
        run_package(
            PackageAction::Outdated,
            ecosystem.as_deref(),
            root.as_deref(),
        )
    }

    /// Check for security vulnerabilities
    ///
    /// Examples:
    ///   normalize package audit                          # audit all ecosystems for vulnerabilities
    ///   normalize package audit -e cargo                  # audit only Cargo dependencies
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
        let _ = (pretty, compact);
        run_package(PackageAction::Audit, ecosystem.as_deref(), root.as_deref())
    }
}
