//! Grammar management commands.

use crate::output::OutputFormatter;
use crate::parsers;
use clap::Subcommand;
use serde::Serialize;
use std::io::Read;
use std::path::PathBuf;

/// Grammar list report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarListReport {
    grammars: Vec<String>,
}

impl OutputFormatter for GrammarListReport {
    fn format_text(&self) -> String {
        if self.grammars.is_empty() {
            let mut lines = vec!["No grammars installed.".to_string(), String::new()];
            lines.push("Install grammars with: moss grammars install".to_string());
            lines.push(
                "Or set MOSS_GRAMMAR_PATH to a directory containing .so/.dylib files".to_string(),
            );
            lines.join("\n")
        } else {
            let mut lines = vec![format!("Installed grammars ({}):", self.grammars.len())];
            for name in &self.grammars {
                lines.push(name.clone());
            }
            lines.join("\n")
        }
    }
}

/// Grammar path item
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct GrammarPath {
    source: String,
    path: String,
    exists: bool,
}

/// Grammar paths report
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GrammarPathsReport {
    paths: Vec<GrammarPath>,
}

impl OutputFormatter for GrammarPathsReport {
    fn format_text(&self) -> String {
        let mut lines = vec!["Grammar search paths:".to_string()];
        for item in &self.paths {
            let exists = if item.exists { "" } else { " (not found)" };
            lines.push(format!("  [{}] {}{}", item.source, item.path, exists));
        }
        lines.join("\n")
    }
}

#[derive(Subcommand, serde::Deserialize, schemars::JsonSchema)]
pub enum GrammarAction {
    /// List installed grammars
    List,

    /// Install grammars from GitHub release
    Install {
        /// Specific version to install (default: latest)
        #[arg(long)]
        version: Option<String>,

        /// Force reinstall even if grammars exist
        #[arg(long)]
        #[serde(default)]
        force: bool,
    },

    /// Show grammar search paths
    Paths,
}

/// Print JSON schema for the command's input arguments.
pub fn print_input_schema() {
    let schema = schemars::schema_for!(GrammarAction);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).unwrap_or_default()
    );
}

/// Run the grammars command
pub fn cmd_grammars(
    action: GrammarAction,
    format: &crate::output::OutputFormat,
    output_schema: bool,
    input_schema: bool,
    params_json: Option<&str>,
) -> i32 {
    let json = format.is_json();
    if input_schema {
        print_input_schema();
        return 0;
    }
    // Override action with --params-json if provided
    let action = match params_json {
        Some(json_str) => match serde_json::from_str(json_str) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("error: invalid --params-json: {}", e);
                return 1;
            }
        },
        None => action,
    };
    if output_schema {
        match action {
            GrammarAction::List => {
                crate::output::print_output_schema::<GrammarListReport>();
            }
            GrammarAction::Paths => {
                crate::output::print_output_schema::<GrammarPathsReport>();
            }
            GrammarAction::Install { .. } => {
                eprintln!("Install subcommand does not have a structured output schema");
                return 1;
            }
        }
        return 0;
    }
    match action {
        GrammarAction::List => cmd_list(format),
        GrammarAction::Install { version, force } => cmd_install(version, force, json),
        GrammarAction::Paths => cmd_paths(format),
    }
}

fn cmd_list(format: &crate::output::OutputFormat) -> i32 {
    let grammars = parsers::available_external_grammars();

    let report = GrammarListReport { grammars };
    report.print(format);

    0
}

fn cmd_paths(format: &crate::output::OutputFormat) -> i32 {
    let mut raw_paths = Vec::new();

    // Environment variable
    if let Ok(env_path) = std::env::var("MOSS_GRAMMAR_PATH") {
        for p in env_path.split(':') {
            if !p.is_empty() {
                raw_paths.push(("env", PathBuf::from(p)));
            }
        }
    }

    // User config directory
    if let Some(config) = dirs::config_dir() {
        raw_paths.push(("config", config.join("moss/grammars")));
    }

    let paths: Vec<GrammarPath> = raw_paths
        .iter()
        .map(|(source, path)| GrammarPath {
            source: source.to_string(),
            path: path.display().to_string(),
            exists: path.exists(),
        })
        .collect();

    let report = GrammarPathsReport { paths };
    report.print(format);

    0
}

fn cmd_install(version: Option<String>, force: bool, json: bool) -> i32 {
    const GITHUB_REPO: &str = "rhi-zone/normalize";

    // Determine install directory
    let install_dir = match dirs::config_dir() {
        Some(config) => config.join("moss/grammars"),
        None => {
            eprintln!("Could not determine config directory");
            return 1;
        }
    };

    // Check if grammars already exist
    if install_dir.exists()
        && !force
        && let Ok(entries) = std::fs::read_dir(&install_dir)
    {
        let count = entries.filter(|e| e.is_ok()).count();
        if count > 0 {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "already_installed",
                        "path": install_dir.display().to_string(),
                        "count": count
                    })
                );
            } else {
                println!(
                    "Grammars already installed at {} ({} files)",
                    install_dir.display(),
                    count
                );
                println!("Use --force to reinstall");
            }
            return 0;
        }
    }

    let client = ureq::agent();

    // Fetch release info
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

    if !json {
        println!("Fetching release info...");
    }

    let response = match client
        .get(&release_url)
        .set("User-Agent", "moss-cli")
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to fetch release: {}", e);
            return 1;
        }
    };

    let body: serde_json::Value = match response.into_json() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to parse response: {}", e);
            return 1;
        }
    };

    let version = body["tag_name"].as_str().unwrap_or("unknown").to_string();

    // Find grammar asset for this platform
    let target = get_target_triple();
    let asset_name = format!("moss-grammars-{}.tar.gz", target);

    let assets = body["assets"].as_array();
    let asset_url = assets.and_then(|arr| {
        arr.iter()
            .find(|a| a["name"].as_str() == Some(&asset_name))
            .and_then(|a| a["browser_download_url"].as_str())
    });

    let asset_url = match asset_url {
        Some(url) => url,
        None => {
            eprintln!("No grammars available for your platform: {}", target);
            eprintln!("Available assets:");
            if let Some(arr) = assets {
                for a in arr {
                    if let Some(name) = a["name"].as_str()
                        && name.contains("grammars")
                    {
                        eprintln!("  - {}", name);
                    }
                }
            }
            return 1;
        }
    };

    // Download grammars
    if !json {
        println!("Downloading {} grammars...", version);
    }

    let archive_response = match client.get(asset_url).call() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to download grammars: {}", e);
            return 1;
        }
    };

    let mut archive_data = Vec::new();
    if let Err(e) = archive_response
        .into_reader()
        .read_to_end(&mut archive_data)
    {
        eprintln!("Failed to read download: {}", e);
        return 1;
    }

    // Create install directory
    if let Err(e) = std::fs::create_dir_all(&install_dir) {
        eprintln!("Failed to create directory: {}", e);
        return 1;
    }

    // Extract grammars
    if !json {
        println!("Installing to {}...", install_dir.display());
    }

    let count = match extract_grammars(&archive_data, &install_dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to extract grammars: {}", e);
            return 1;
        }
    };

    if json {
        println!(
            "{}",
            serde_json::json!({
                "status": "installed",
                "version": version,
                "path": install_dir.display().to_string(),
                "count": count
            })
        );
    } else {
        println!("Installed {} grammars from {}", count, version);
    }

    0
}

fn get_target_triple() -> String {
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    let os = if cfg!(target_os = "linux") {
        "unknown-linux-gnu"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "windows") {
        "pc-windows-msvc"
    } else {
        "unknown"
    };

    format!("{}-{}", arch, os)
}

fn extract_grammars(data: &[u8], dest: &std::path::Path) -> Result<usize, String> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    let mut count = 0;

    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;

        // Only extract .so, .dylib, or .dll files
        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".so")
                || name_str.ends_with(".dylib")
                || name_str.ends_with(".dll")
            {
                let dest_path = dest.join(name);
                entry.unpack(&dest_path).map_err(|e| e.to_string())?;
                count += 1;
            }
        }
    }

    Ok(count)
}
