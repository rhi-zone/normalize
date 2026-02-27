//! Grammar management service for server-less CLI.

use crate::commands::grammars::{GrammarListReport, GrammarPathsReport};
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;

/// Grammar management sub-service.
pub struct GrammarService {
    pretty: Cell<bool>,
}

impl GrammarService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
        }
    }

    fn display_list(&self, value: &GrammarListReport) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
        }
    }

    fn display_paths(&self, value: &GrammarPathsReport) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
        }
    }
}

/// Install result for grammar installation.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct GrammarInstallResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub path: String,
    pub count: usize,
}

impl std::fmt::Display for GrammarInstallResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.status == "already_installed" {
            write!(
                f,
                "Grammars already installed at {} ({} files)\nUse --force to reinstall",
                self.path, self.count
            )
        } else {
            write!(
                f,
                "Installed {} grammars from {}",
                self.count,
                self.version.as_deref().unwrap_or("unknown")
            )
        }
    }
}

#[cli(name = "grammars", about = "Manage tree-sitter grammars for parsing")]
impl GrammarService {
    /// List installed grammars
    #[cli(display_with = "display_list")]
    pub fn list(&self) -> Result<GrammarListReport, String> {
        let grammars = crate::parsers::available_external_grammars();
        Ok(GrammarListReport::new(grammars))
    }

    /// Install grammars from GitHub release
    pub fn install(
        &self,
        #[param(help = "Specific version to install (default: latest)")] version: Option<String>,
        #[param(help = "Force reinstall even if grammars exist")] force: bool,
    ) -> Result<GrammarInstallResult, String> {
        crate::commands::grammars::cmd_install_service(version, force)
    }

    /// Show grammar search paths
    #[cli(display_with = "display_paths")]
    pub fn paths(&self) -> Result<GrammarPathsReport, String> {
        Ok(crate::commands::grammars::build_paths_report())
    }
}
