//! Grammar management service for server-less CLI.

use crate::commands::grammars::{GrammarListReport, GrammarPathsReport};
use crate::output::OutputFormatter;
use server_less::cli;
use std::cell::Cell;
use std::io::Read as _;

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

    /// Generic display bridge: routes to `format_pretty()` or `format_text()` based on pretty mode.
    fn display_output<T: crate::output::OutputFormatter>(&self, value: &T) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
        }
    }
}

/// Report for grammar installation.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct GrammarInstallReport {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub path: String,
    pub count: usize,
    pub dry_run: bool,
}

impl OutputFormatter for GrammarInstallReport {
    fn format_text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        if self.dry_run {
            let _ = write!(out, "[dry-run] Would install grammars to {}", self.path);
            if let Some(ref v) = self.version {
                let _ = write!(out, " (version {})", v);
            }
            if self.status == "already_installed" {
                let _ = write!(
                    out,
                    "\n  ({} files already present, use --force to reinstall)",
                    self.count
                );
            }
        } else if self.status == "already_installed" {
            let _ = write!(
                out,
                "Grammars already installed at {} ({} files)\nUse --force to reinstall",
                self.path, self.count
            );
        } else {
            let _ = write!(
                out,
                "Installed {} grammars from {}",
                self.count,
                self.version.as_deref().unwrap_or("unknown")
            );
        }
        out
    }
}

#[cli(
    name = "grammars",
    description = "Install and list tree-sitter grammars. Run after install or when parsing fails for a language."
)]
impl GrammarService {
    /// List installed grammars
    ///
    /// Examples:
    ///   normalize grammars list          # show all available tree-sitter grammars
    ///   normalize grammars list --json   # machine-readable grammar list
    #[cli(display_with = "display_output")]
    pub fn list(&self) -> Result<GrammarListReport, String> {
        let grammars = crate::parsers::available_external_grammars_with_paths();
        Ok(GrammarListReport::new(grammars))
    }

    /// Install grammars from GitHub release
    ///
    /// Examples:
    ///   normalize grammars install                    # install latest grammars
    ///   normalize grammars install --version v0.1.0   # install a specific version
    ///   normalize grammars install --force             # reinstall even if grammars exist
    #[cli(display_with = "display_output")]
    pub fn install(
        &self,
        #[param(help = "Specific version to install (default: latest)")] version: Option<String>,
        #[param(help = "Force reinstall even if grammars exist")] force: bool,
        #[param(help = "Preview what would be installed without downloading")] dry_run: bool,
    ) -> Result<GrammarInstallReport, String> {
        use crate::commands::update::get_target_triple;

        const GITHUB_REPO: &str = "rhi-zone/normalize";

        let install_dir = dirs::config_dir()
            .map(|c| c.join("normalize/grammars"))
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        // Check if grammars already exist
        if install_dir.exists()
            && !force
            && let Ok(entries) = std::fs::read_dir(&install_dir)
        {
            let count = entries.filter(|e| e.is_ok()).count();
            if count > 0 {
                return Ok(GrammarInstallReport {
                    status: "already_installed".to_string(),
                    version,
                    path: install_dir.display().to_string(),
                    count,
                    dry_run,
                });
            }
        }

        if dry_run {
            return Ok(GrammarInstallReport {
                status: "would_install".to_string(),
                version: version.or_else(|| Some("latest".to_string())),
                path: install_dir.display().to_string(),
                count: 0,
                dry_run,
            });
        }

        use flate2::read::GzDecoder;
        use tar::Archive;

        let client = ureq::agent();

        let release_url = match &version {
            Some(v) => format!(
                "https://api.github.com/repos/{}/releases/tags/{}",
                GITHUB_REPO, v
            ),
            None => format!(
                "https://api.github.com/repos/{}/releases/latest",
                GITHUB_REPO
            ),
        };

        eprintln!("Fetching release info...");

        let response = client
            .get(&release_url)
            .set("User-Agent", "normalize-cli")
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| format!("Failed to fetch release: {}", e))?;

        let body: serde_json::Value = response
            .into_json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let version_str = body["tag_name"].as_str().unwrap_or("unknown").to_string();

        let target = get_target_triple();
        let asset_name = format!("normalize-grammars-{}.tar.gz", target);

        let assets = body["assets"].as_array();
        let asset_url = assets
            .and_then(|arr| {
                arr.iter()
                    .find(|a| a["name"].as_str() == Some(&asset_name))
                    .and_then(|a| a["browser_download_url"].as_str())
            })
            .ok_or_else(|| format!("No grammars available for your platform: {}", target))?;

        eprintln!("Downloading {} grammars...", version_str);

        let archive_response = client
            .get(asset_url)
            .call()
            .map_err(|e| format!("Failed to download grammars: {}", e))?;

        let mut archive_data = Vec::new();
        archive_response
            .into_reader()
            .read_to_end(&mut archive_data)
            .map_err(|e| format!("Failed to read download: {}", e))?;

        std::fs::create_dir_all(&install_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        eprintln!("Installing to {}...", install_dir.display());

        // Extract grammar shared libraries from the archive
        let decoder = GzDecoder::new(archive_data.as_slice());
        let mut archive = Archive::new(decoder);
        let mut count = 0;

        for entry in archive.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?;

            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.ends_with(".so")
                    || name_str.ends_with(".dylib")
                    || name_str.ends_with(".dll")
                {
                    let dest_path = install_dir.join(name);
                    entry.unpack(&dest_path).map_err(|e| e.to_string())?;
                    count += 1;
                }
            }
        }

        Ok(GrammarInstallReport {
            status: "installed".to_string(),
            version: Some(version_str),
            path: install_dir.display().to_string(),
            count,
            dry_run,
        })
    }

    /// Show grammar search paths
    ///
    /// Examples:
    ///   normalize grammars paths          # show directories searched for grammar .so files
    #[cli(display_with = "display_output")]
    pub fn paths(&self) -> Result<GrammarPathsReport, String> {
        Ok(crate::commands::grammars::build_paths_report())
    }
}
