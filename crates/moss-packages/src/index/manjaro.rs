//! Manjaro Linux package index fetcher.
//!
//! Fetches package metadata from Manjaro repositories.
//! Uses the search.manjaro-sway.download API.
//!
//! ## API Strategy
//! - **fetch**: `search.manjaro-sway.download/{name}` - Community JSON API + AUR fallback
//! - **fetch_versions**: Same API, single version
//! - **search**: `search.manjaro-sway.download/search?q=`
//! - **fetch_all**: AUR `packages-meta-ext-v1.json.gz` via arch_common

use super::arch_common;
use super::{IndexError, PackageIndex, PackageMeta, VersionMeta};
use std::collections::HashMap;

/// Manjaro Linux package index fetcher.
pub struct Manjaro;

impl Manjaro {
    /// Manjaro package search API (community-maintained).
    const MANJARO_API: &'static str = "https://search.manjaro-sway.download/";

    /// Arch AUR (Manjaro users can also use AUR packages).
    const AUR_RPC: &'static str = "https://aur.archlinux.org/rpc/";

    /// Parse package from Manjaro API response.
    fn parse_package(pkg: &serde_json::Value, branch: &str) -> Option<PackageMeta> {
        let branch_data = pkg.get(branch)?;
        let name = branch_data["name"].as_str()?;
        let version = branch_data["version"].as_str()?;

        let mut extra = HashMap::new();

        // Extract dependencies
        if let Some(deps) = branch_data["depends"].as_array() {
            let parsed_deps: Vec<serde_json::Value> = deps
                .iter()
                .filter_map(|d| d.as_str())
                .map(|d| {
                    // Strip version constraints: "libc6>=2.17" -> "libc6"
                    let name = d
                        .split(|c| c == '>' || c == '<' || c == '=' || c == ':')
                        .next()
                        .unwrap_or(d);
                    serde_json::Value::String(name.to_string())
                })
                .collect();
            extra.insert("depends".to_string(), serde_json::Value::Array(parsed_deps));
        }

        // Extract size
        if let Some(csize) = branch_data["csize"].as_str() {
            if let Ok(size) = csize.parse::<u64>() {
                extra.insert("size".to_string(), serde_json::Value::Number(size.into()));
            }
        }

        // Build archive URL from Manjaro mirrors
        let filename = branch_data["filename"].as_str()?;
        let arch = branch_data["arch"].as_str().unwrap_or("x86_64");
        // Use Manjaro's mirror, mapping branch to repo path
        let repo = match branch {
            "stable_x86_64" | "stable_aarch64" => "stable",
            "testing_x86_64" | "testing_aarch64" => "testing",
            "unstable_x86_64" | "unstable_aarch64" => "unstable",
            _ => "stable",
        };
        let archive_url = format!(
            "https://mirror.manjaro.org/{}/extra/{}/{}",
            repo, arch, filename
        );

        // Extract checksum
        let checksum = branch_data["sha256sum"]
            .as_str()
            .map(|s| format!("sha256:{}", s));

        Some(PackageMeta {
            name: name.to_string(),
            version: version.to_string(),
            description: branch_data["desc"].as_str().map(String::from),
            homepage: branch_data["url"].as_str().map(String::from),
            repository: None,
            license: branch_data["license"].as_str().map(String::from),
            binaries: Vec::new(),
            archive_url: Some(archive_url),
            checksum,
            extra,
            ..Default::default()
        })
    }
}

impl PackageIndex for Manjaro {
    fn ecosystem(&self) -> &'static str {
        "manjaro"
    }

    fn display_name(&self) -> &'static str {
        "Manjaro Linux"
    }

    fn fetch(&self, name: &str) -> Result<PackageMeta, IndexError> {
        // Try Manjaro API first
        let url = format!("{}?q={}&size=10", Self::MANJARO_API, name);
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        if let Some(results) = response["result"].as_array() {
            // Find exact match
            for pkg in results {
                if pkg["name"].as_str() == Some(name) {
                    // Try stable branch first, then testing, then unstable
                    for branch in ["stable_x86_64", "testing_x86_64", "unstable_x86_64"] {
                        if let Some(meta) = Self::parse_package(pkg, branch) {
                            return Ok(meta);
                        }
                    }
                }
            }
        }

        // Fall back to AUR
        arch_common::fetch_aur(Self::AUR_RPC, name)
    }

    fn fetch_versions(&self, name: &str) -> Result<Vec<VersionMeta>, IndexError> {
        // Manjaro API shows per-branch versions
        let url = format!("{}?q={}&size=10", Self::MANJARO_API, name);
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        let mut versions = Vec::new();

        if let Some(results) = response["result"].as_array() {
            for pkg in results {
                if pkg["name"].as_str() == Some(name) {
                    // Collect versions from all branches
                    for branch in [
                        "stable_x86_64",
                        "testing_x86_64",
                        "unstable_x86_64",
                        "stable_aarch64",
                    ] {
                        if let Some(branch_data) = pkg.get(branch) {
                            if let Some(version) = branch_data["version"].as_str() {
                                // Avoid duplicates
                                if !versions.iter().any(|v: &VersionMeta| v.version == version) {
                                    versions.push(VersionMeta {
                                        version: version.to_string(),
                                        released: None,
                                        yanked: false,
                                    });
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }

        if versions.is_empty() {
            // Try AUR
            let pkg = arch_common::fetch_aur(Self::AUR_RPC, name)?;
            versions.push(VersionMeta {
                version: pkg.version,
                released: None,
                yanked: false,
            });
        }

        Ok(versions)
    }

    fn search(&self, query: &str) -> Result<Vec<PackageMeta>, IndexError> {
        let url = format!("{}?q={}&size=50", Self::MANJARO_API, query);
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        let mut packages = Vec::new();

        if let Some(results) = response["result"].as_array() {
            for pkg in results {
                // Try stable branch first
                for branch in ["stable_x86_64", "testing_x86_64", "unstable_x86_64"] {
                    if let Some(meta) = Self::parse_package(pkg, branch) {
                        packages.push(meta);
                        break;
                    }
                }
            }
        }

        // Also search AUR
        if let Ok(aur_packages) = arch_common::search_aur(Self::AUR_RPC, query) {
            packages.extend(aur_packages);
        }

        Ok(packages)
    }
}
