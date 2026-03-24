//! Package management service for server-less CLI.

use crate::commands::package::{
    find_dependency_paths, print_audit_human, print_human, print_tree, show_outdated_data,
};
use crate::output::OutputFormatter;
use normalize_ecosystems::{Dependency, DependencyTree, PackageInfo, Vulnerability};
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

    /// Generic display bridge that routes to `OutputFormatter::format_text()`.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        value.format_text()
    }
}

// ── Report types ─────────────────────────────────────────────────────────────

/// Report for `normalize package info`: package metadata from a registry.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PackageInfoReport {
    /// The ecosystem that resolved this package (e.g. "cargo", "npm").
    pub ecosystem: String,
    /// Package metadata returned by the registry.
    pub info: PackageInfo,
}

impl OutputFormatter for PackageInfoReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&print_human(&self.info, &self.ecosystem));
        out
    }
}

/// Report for `normalize package list`: declared dependencies from a manifest.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PackageListReport {
    /// The ecosystem whose manifest was read (e.g. "cargo", "npm").
    pub ecosystem: String,
    /// Declared dependencies with names and version requirements.
    pub packages: Vec<Dependency>,
}

impl OutputFormatter for PackageListReport {
    fn format_text(&self) -> String {
        let mut out = format!(
            "{} dependencies ({})\n\n",
            self.packages.len(),
            self.ecosystem
        );
        for dep in &self.packages {
            let version = dep.version_req.as_deref().unwrap_or("*");
            let optional = if dep.optional { " (optional)" } else { "" };
            out.push_str(&format!("  {} {}{}\n", dep.name, version, optional));
        }
        out.trim_end().to_string()
    }
}

/// Report for `normalize package tree`: full dependency tree from the lockfile.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PackageTreeReport {
    /// The ecosystem whose lockfile was parsed (e.g. "cargo", "npm").
    pub ecosystem: String,
    /// Dependency tree rooted at the direct dependencies.
    pub tree: DependencyTree,
}

impl OutputFormatter for PackageTreeReport {
    fn format_text(&self) -> String {
        print_tree(&self.tree)
    }
}

/// One entry in the dependency path for `normalize package why`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct DependencyPathEntry {
    /// Package name.
    pub name: String,
    /// Package version (may be empty if not known).
    pub version: String,
}

/// Report for `normalize package why`: all paths from roots to the queried package.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PackageWhyReport {
    /// The package that was traced.
    pub package: String,
    /// The ecosystem searched (e.g. "cargo", "npm").
    pub ecosystem: String,
    /// All dependency paths from root packages to the queried package.
    pub paths: Vec<Vec<DependencyPathEntry>>,
}

impl OutputFormatter for PackageWhyReport {
    fn format_text(&self) -> String {
        if self.paths.is_empty() {
            return format!("Package '{}' not found in dependency tree", self.package);
        }

        let mut out = format!(
            "'{}' is required by {} path(s):\n",
            self.package,
            self.paths.len()
        );
        for (i, path) in self.paths.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push('\n');
            for (j, entry) in path.iter().enumerate() {
                let indent = "  ".repeat(j);
                if entry.version.is_empty() {
                    out.push_str(&format!("{}{}\n", indent, entry.name));
                } else {
                    out.push_str(&format!("{}{} v{}\n", indent, entry.name, entry.version));
                }
            }
        }
        out.trim_end().to_string()
    }
}

/// A single outdated package entry.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct OutdatedEntry {
    /// Package name.
    pub name: String,
    /// Currently installed version, if known.
    pub installed: Option<String>,
    /// Latest available version.
    pub latest: String,
    /// Version constraint from the manifest.
    pub wanted: Option<String>,
}

/// Report for `normalize package outdated`: list of packages with newer versions available.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PackageOutdatedReport {
    /// The ecosystem checked (e.g. "cargo", "npm").
    pub ecosystem: String,
    /// Packages that have a newer version available.
    pub outdated: Vec<OutdatedEntry>,
    /// Packages that could not be queried, with their error messages.
    pub errors: Vec<(String, String)>,
}

impl OutputFormatter for PackageOutdatedReport {
    fn format_text(&self) -> String {
        if self.outdated.is_empty() && self.errors.is_empty() {
            return "All packages are up to date".to_string();
        }

        let mut out = String::new();
        if !self.outdated.is_empty() {
            out.push_str(&format!("Outdated packages ({}):\n\n", self.outdated.len()));
            for pkg in &self.outdated {
                let installed = pkg.installed.as_deref().unwrap_or("(not installed)");
                out.push_str(&format!("  {} {} → {}\n", pkg.name, installed, pkg.latest));
            }
        }
        if !self.errors.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("Errors ({}):\n", self.errors.len()));
            for (name, err) in &self.errors {
                out.push_str(&format!("  {}: {}\n", name, err));
            }
        }
        out.trim_end().to_string()
    }
}

/// Report for `normalize package audit`: security vulnerabilities found in dependencies.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PackageAuditReport {
    /// The ecosystem audited (e.g. "cargo", "npm").
    pub ecosystem: String,
    /// Vulnerabilities found during the audit.
    pub vulnerabilities: Vec<Vulnerability>,
}

impl OutputFormatter for PackageAuditReport {
    fn format_text(&self) -> String {
        print_audit_human(&self.vulnerabilities, &self.ecosystem)
    }
}

// ── Service impl ──────────────────────────────────────────────────────────────

#[cli(
    name = "package",
    description = "Package management: info, list, tree, outdated",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
)]
impl PackageService {
    /// Query package info from registry
    ///
    /// Examples:
    ///   normalize package info express                    # look up a package by name
    ///   normalize package info serde@1.0 -e cargo         # query a specific version and ecosystem
    #[cli(display_with = "display_output")]
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
    ) -> Result<PackageInfoReport, String> {
        let _ = (pretty, compact);
        let root_path = root.as_deref().map(Path::new).unwrap_or(Path::new("."));
        let (eco, info) =
            crate::commands::package::get_info(&package, ecosystem.as_deref(), root_path)?;
        Ok(PackageInfoReport {
            ecosystem: eco,
            info,
        })
    }

    /// List declared dependencies from manifest
    ///
    /// Examples:
    ///   normalize package list                           # list dependencies for current project
    ///   normalize package list -e npm                    # list only npm dependencies
    #[cli(display_with = "display_output")]
    pub fn list(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageListReport, String> {
        let _ = (pretty, compact);
        let root_path = root.as_deref().map(Path::new).unwrap_or(Path::new("."));
        let (eco, packages) = crate::commands::package::get_list(ecosystem.as_deref(), root_path)?;
        Ok(PackageListReport {
            ecosystem: eco,
            packages,
        })
    }

    /// Show dependency tree from lockfile
    ///
    /// Examples:
    ///   normalize package tree                           # show full dependency tree
    ///   normalize package tree -e cargo                  # show only Cargo dependency tree
    #[cli(display_with = "display_output")]
    pub fn tree(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageTreeReport, String> {
        let _ = (pretty, compact);
        let root_path = root.as_deref().map(Path::new).unwrap_or(Path::new("."));
        let (eco, tree) = crate::commands::package::get_tree(ecosystem.as_deref(), root_path)?;
        Ok(PackageTreeReport {
            ecosystem: eco,
            tree,
        })
    }

    /// Show why a dependency is in the tree
    ///
    /// Examples:
    ///   normalize package why serde                      # trace why serde is a dependency
    #[cli(display_with = "display_output")]
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
    ) -> Result<PackageWhyReport, String> {
        let _ = (pretty, compact);
        let root_path = root.as_deref().map(Path::new).unwrap_or(Path::new("."));
        let (eco, tree) = crate::commands::package::get_tree(ecosystem.as_deref(), root_path)?;
        let raw_paths = find_dependency_paths(&tree, &package);
        let paths = raw_paths
            .into_iter()
            .map(|path| {
                path.into_iter()
                    .map(|(name, version)| DependencyPathEntry { name, version })
                    .collect()
            })
            .collect();
        Ok(PackageWhyReport {
            package,
            ecosystem: eco,
            paths,
        })
    }

    /// Show outdated packages (installed vs latest)
    ///
    /// Examples:
    ///   normalize package outdated                       # check all ecosystems for outdated deps
    ///   normalize package outdated -e npm                # check only npm packages
    #[cli(display_with = "display_output")]
    pub fn outdated(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageOutdatedReport, String> {
        let _ = (pretty, compact);
        let root_path = root.as_deref().map(Path::new).unwrap_or(Path::new("."));
        let (eco, outdated, errors) = show_outdated_data(ecosystem.as_deref(), root_path)?;
        let outdated = outdated
            .into_iter()
            .map(|(name, installed, latest, wanted)| OutdatedEntry {
                name,
                installed,
                latest,
                wanted,
            })
            .collect();
        Ok(PackageOutdatedReport {
            ecosystem: eco,
            outdated,
            errors,
        })
    }

    /// Check for security vulnerabilities
    ///
    /// Examples:
    ///   normalize package audit                          # audit all ecosystems for vulnerabilities
    ///   normalize package audit -e cargo                  # audit only Cargo dependencies
    #[cli(display_with = "display_output")]
    pub fn audit(
        &self,
        #[param(short = 'e', help = "Force specific ecosystem (cargo, npm, python)")]
        ecosystem: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<PackageAuditReport, String> {
        let _ = (pretty, compact);
        let root_path = root.as_deref().map(Path::new).unwrap_or(Path::new("."));
        let (eco, vulns) = crate::commands::package::get_audit(ecosystem.as_deref(), root_path)?;
        Ok(PackageAuditReport {
            ecosystem: eco,
            vulnerabilities: vulns,
        })
    }
}
