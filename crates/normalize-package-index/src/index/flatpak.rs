//! Flathub package index fetcher (Flatpak apps).
//!
//! Fetches package metadata from Flathub API.
//!
//! ## API Strategy
//! - **fetch**: `flathub.org/api/v2/appstream/{app_id}` - Official Flathub JSON API
//! - **fetch_versions**: Same API, extracts releases array
//! - **search**: `flathub.org/api/v2/search?q=` - Flathub search
//! - **fetch_all**: `flathub.org/api/v2/appstream` (all apps)
//!
//! ## Multi-remote Support
//! ```rust,ignore
//! use normalize_packages::index::flatpak::{Flatpak, FlatpakRemote};
//!
//! // All remotes (default)
//! let all = Flatpak::all();
//!
//! // Flathub only
//! let flathub = Flatpak::flathub();
//! ```

use super::{IndexError, PackageIndex, PackageMeta, VersionMeta};
use std::collections::HashMap;

/// Available Flatpak remotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlatpakRemote {
    /// Flathub - the main Flatpak repository
    Flathub,
    /// Flathub Beta - testing versions
    FlathubBeta,
}

impl FlatpakRemote {
    /// Get the API base URL for this remote.
    fn api_url(&self) -> &'static str {
        match self {
            Self::Flathub => "https://flathub.org/api/v2",
            Self::FlathubBeta => "https://beta.flathub.org/api/v2",
        }
    }

    /// Get the remote name for tagging.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Flathub => "flathub",
            Self::FlathubBeta => "flathub-beta",
        }
    }

    /// All available remotes.
    pub fn all() -> &'static [FlatpakRemote] {
        &[Self::Flathub, Self::FlathubBeta]
    }

    /// Flathub stable only.
    pub fn flathub() -> &'static [FlatpakRemote] {
        &[Self::Flathub]
    }
}

/// Flathub package index fetcher with configurable remotes.
pub struct Flatpak {
    remotes: Vec<FlatpakRemote>,
}

impl Flatpak {
    /// Create a fetcher with all remotes.
    pub fn all() -> Self {
        Self {
            remotes: FlatpakRemote::all().to_vec(),
        }
    }

    /// Create a fetcher with Flathub stable only.
    pub fn flathub() -> Self {
        Self {
            remotes: FlatpakRemote::flathub().to_vec(),
        }
    }

    /// Create a fetcher with custom remote selection.
    pub fn with_remotes(remotes: &[FlatpakRemote]) -> Self {
        Self {
            remotes: remotes.to_vec(),
        }
    }

    /// Fetch an app from a specific remote.
    fn fetch_from_remote(name: &str, remote: FlatpakRemote) -> Result<PackageMeta, IndexError> {
        let url = format!("{}/appstream/{}", remote.api_url(), name);
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        Ok(app_to_meta(&response, name, remote))
    }

    /// Fetch versions from a specific remote.
    fn fetch_versions_from_remote(
        name: &str,
        remote: FlatpakRemote,
    ) -> Result<Vec<VersionMeta>, IndexError> {
        let url = format!("{}/appstream/{}", remote.api_url(), name);
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        // Flathub typically has only the current version
        let version = response["releases"]
            .as_array()
            .and_then(|r| r.first())
            .and_then(|r| r["version"].as_str())
            .or_else(|| response["bundle"]["runtime"].as_str())
            .unwrap_or("unknown");

        Ok(vec![VersionMeta {
            version: format!("{} ({})", version, remote.name()),
            released: response["releases"]
                .as_array()
                .and_then(|r| r.first())
                .and_then(|r| r["timestamp"].as_str())
                .map(String::from),
            yanked: false,
        }])
    }

    /// Search a specific remote.
    fn search_remote(query: &str, remote: FlatpakRemote) -> Result<Vec<PackageMeta>, IndexError> {
        let url = format!(
            "{}/search?q={}",
            remote.api_url(),
            urlencoding::encode(query)
        );
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        let hits = response["hits"]
            .as_array()
            .ok_or_else(|| IndexError::Parse("missing hits".into()))?;

        Ok(hits
            .iter()
            .take(50)
            .filter_map(|hit| {
                let mut extra = HashMap::new();
                extra.insert(
                    "source_repo".to_string(),
                    serde_json::Value::String(remote.name().to_string()),
                );

                Some(PackageMeta {
                    name: hit["id"].as_str()?.to_string(),
                    version: "unknown".to_string(),
                    description: hit["summary"].as_str().map(String::from),
                    homepage: hit["project_url"].as_str().map(String::from),
                    repository: None,
                    license: None,
                    binaries: Vec::new(),
                    keywords: Vec::new(),
                    maintainers: Vec::new(),
                    published: None,
                    downloads: None,
                    archive_url: None,
                    checksum: None,
                    extra,
                })
            })
            .collect())
    }

    /// Fetch all apps from a specific remote.
    fn fetch_all_from_remote(remote: FlatpakRemote) -> Result<Vec<PackageMeta>, IndexError> {
        let url = format!("{}/appstream", remote.api_url());
        let app_ids: Vec<String> = ureq::get(&url).call()?.into_json()?;

        let mut extra = HashMap::new();
        extra.insert(
            "source_repo".to_string(),
            serde_json::Value::String(remote.name().to_string()),
        );

        Ok(app_ids
            .into_iter()
            .map(|id| PackageMeta {
                name: id,
                version: "unknown".to_string(),
                description: None,
                homepage: None,
                repository: None,
                license: None,
                binaries: Vec::new(),
                keywords: Vec::new(),
                maintainers: Vec::new(),
                published: None,
                downloads: None,
                archive_url: None,
                checksum: None,
                extra: extra.clone(),
            })
            .collect())
    }
}

impl PackageIndex for Flatpak {
    fn ecosystem(&self) -> &'static str {
        "flatpak"
    }

    fn display_name(&self) -> &'static str {
        "Flathub (Flatpak)"
    }

    fn fetch(&self, name: &str) -> Result<PackageMeta, IndexError> {
        // Try each configured remote until we find the app
        for &remote in &self.remotes {
            match Self::fetch_from_remote(name, remote) {
                Ok(pkg) => return Ok(pkg),
                Err(IndexError::Network(_)) => continue,
                Err(e) => return Err(e),
            }
        }

        Err(IndexError::NotFound(name.to_string()))
    }

    fn fetch_versions(&self, name: &str) -> Result<Vec<VersionMeta>, IndexError> {
        let mut all_versions = Vec::new();

        for &remote in &self.remotes {
            if let Ok(versions) = Self::fetch_versions_from_remote(name, remote) {
                all_versions.extend(versions);
            }
        }

        if all_versions.is_empty() {
            return Err(IndexError::NotFound(name.to_string()));
        }

        Ok(all_versions)
    }

    fn supports_fetch_all(&self) -> bool {
        true
    }

    fn fetch_all(&self) -> Result<Vec<PackageMeta>, IndexError> {
        let mut all_apps = Vec::new();

        for &remote in &self.remotes {
            if let Ok(apps) = Self::fetch_all_from_remote(remote) {
                all_apps.extend(apps);
            }
        }

        Ok(all_apps)
    }

    fn search(&self, query: &str) -> Result<Vec<PackageMeta>, IndexError> {
        let mut results = Vec::new();

        for &remote in &self.remotes {
            if let Ok(packages) = Self::search_remote(query, remote) {
                results.extend(packages);
            }
        }

        Ok(results)
    }
}

fn app_to_meta(app: &serde_json::Value, fallback_name: &str, remote: FlatpakRemote) -> PackageMeta {
    let version = app["releases"]
        .as_array()
        .and_then(|r| r.first())
        .and_then(|r| r["version"].as_str())
        .unwrap_or("unknown");

    let mut extra = HashMap::new();
    extra.insert(
        "source_repo".to_string(),
        serde_json::Value::String(remote.name().to_string()),
    );

    let published = app["releases"]
        .as_array()
        .and_then(|r| r.first())
        .and_then(|r| r["timestamp"].as_str())
        .map(String::from);

    PackageMeta {
        name: app["id"].as_str().unwrap_or(fallback_name).to_string(),
        version: version.to_string(),
        description: app["summary"].as_str().map(String::from),
        homepage: app["project_url"].as_str().map(String::from),
        repository: app["vcs_url"].as_str().map(String::from),
        license: app["project_license"].as_str().map(String::from),
        binaries: Vec::new(),
        keywords: app["categories"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        maintainers: app["developer_name"]
            .as_str()
            .map(|d| vec![d.to_string()])
            .unwrap_or_default(),
        published,
        downloads: app["installs_last_month"].as_u64(),
        archive_url: None,
        checksum: None,
        extra,
    }
}
