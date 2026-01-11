//! FreeBSD package index fetcher (pkg).
//!
//! Fetches package metadata from FreeBSD package repositories.
//!
//! ## API Strategy
//! - **fetch**: Searches cached `pkg.freebsd.org/.../packagesite.pkg` (zstd tar + JSON-lines)
//! - **fetch_versions**: Same, single version per package
//! - **search**: Filters cached packagesite data
//! - **fetch_all**: Full packagesite.pkg (cached 1 hour, ~60MB uncompressed)

use super::{IndexError, PackageIndex, PackageMeta, VersionMeta};
use crate::cache;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::time::Duration;

/// Cache TTL for FreeBSD package index (1 hour).
const CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// FreeBSD package index fetcher.
pub struct FreeBsd;

impl FreeBsd {
    /// FreeBSD package repository URL.
    const REPO_URL: &'static str =
        "https://pkg.freebsd.org/FreeBSD:14:amd64/latest/packagesite.pkg";

    /// Parse a JSON-lines package entry.
    fn parse_package(line: &str) -> Option<PackageMeta> {
        let pkg: serde_json::Value = serde_json::from_str(line).ok()?;

        let name = pkg["name"].as_str()?;
        let version = pkg["version"].as_str().unwrap_or("unknown");

        let license = pkg["licenses"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|l| l.as_str())
            .map(String::from);

        let mut extra = HashMap::new();
        if let Some(deps) = pkg["deps"].as_object() {
            let dep_names: Vec<serde_json::Value> = deps
                .keys()
                .map(|k| serde_json::Value::String(k.clone()))
                .collect();
            extra.insert("depends".to_string(), serde_json::Value::Array(dep_names));
        }

        Some(PackageMeta {
            name: name.to_string(),
            version: version.to_string(),
            description: pkg["comment"].as_str().map(String::from),
            homepage: pkg["www"].as_str().map(String::from),
            repository: Some("https://www.freshports.org/".to_string()),
            license,
            maintainers: pkg["maintainer"]
                .as_str()
                .map(|m| vec![m.to_string()])
                .unwrap_or_default(),
            binaries: Vec::new(),
            extra,
            ..Default::default()
        })
    }

    /// Load and parse the package index.
    fn load_packages() -> Result<Vec<PackageMeta>, IndexError> {
        let (data, _was_cached) =
            cache::fetch_with_cache("freebsd", "packagesite", Self::REPO_URL, CACHE_TTL)
                .map_err(IndexError::Network)?;

        // Decompress zstd
        let decompressed = zstd::decode_all(std::io::Cursor::new(&data))
            .map_err(|e| IndexError::Decompress(e.to_string()))?;

        // Extract tar
        let mut archive = tar::Archive::new(std::io::Cursor::new(decompressed));
        let mut packages = Vec::new();

        for entry in archive.entries().map_err(IndexError::Io)? {
            let entry = entry.map_err(IndexError::Io)?;
            let path = entry.path().map_err(IndexError::Io)?;
            let path_str = path.to_string_lossy();

            // Match exact filename, not .sig or .pub
            if path_str == "packagesite.yaml" {
                // Parse JSON-lines
                let reader = BufReader::new(entry);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        if !line.is_empty() {
                            if let Some(pkg) = Self::parse_package(&line) {
                                packages.push(pkg);
                            }
                        }
                    }
                }
                break;
            }
        }

        Ok(packages)
    }
}

impl PackageIndex for FreeBsd {
    fn ecosystem(&self) -> &'static str {
        "freebsd"
    }

    fn display_name(&self) -> &'static str {
        "FreeBSD (pkg)"
    }

    fn fetch(&self, name: &str) -> Result<PackageMeta, IndexError> {
        let packages = Self::load_packages()?;

        packages
            .into_iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| IndexError::NotFound(name.to_string()))
    }

    fn fetch_versions(&self, name: &str) -> Result<Vec<VersionMeta>, IndexError> {
        let pkg = self.fetch(name)?;
        Ok(vec![VersionMeta {
            version: pkg.version,
            released: None,
            yanked: false,
        }])
    }

    fn search(&self, query: &str) -> Result<Vec<PackageMeta>, IndexError> {
        let packages = Self::load_packages()?;
        let query_lower = query.to_lowercase();

        Ok(packages
            .into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query_lower)
                    || p.description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
            })
            .take(50)
            .collect())
    }

    fn fetch_all(&self) -> Result<Vec<PackageMeta>, IndexError> {
        Self::load_packages()
    }
}
