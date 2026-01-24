//! Snap package index fetcher (Ubuntu/Linux).
//!
//! Fetches package metadata from the Snapcraft.io API.
//!
//! ## API Strategy
//! - **fetch**: `api.snapcraft.io/v2/snaps/info/{name}` - Official Snapcraft JSON API
//! - **fetch_versions**: Same API, extracts channel-map
//! - **search**: `api.snapcraft.io/v2/snaps/find?q=`
//! - **fetch_all**: Not supported (too large)
//!
//! ## Multi-channel Support
//! ```rust,ignore
//! use rhi_normalize_packages::index::snap::{Snap, SnapChannel};
//!
//! // All channels (default)
//! let all = Snap::all();
//!
//! // Stable only
//! let stable = Snap::stable();
//!
//! // Development channels
//! let dev = Snap::dev();
//! ```

use super::{IndexError, PackageIndex, PackageMeta, VersionMeta};
use std::collections::{HashMap, HashSet};

/// Available Snap channels/tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapChannel {
    /// Stable - production-ready releases
    Stable,
    /// Candidate - release candidates
    Candidate,
    /// Beta - beta releases
    Beta,
    /// Edge - development/nightly builds
    Edge,
}

impl SnapChannel {
    /// Get the channel risk level name.
    pub fn risk(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Candidate => "candidate",
            Self::Beta => "beta",
            Self::Edge => "edge",
        }
    }

    /// Get the channel name for tagging.
    pub fn name(&self) -> &'static str {
        self.risk()
    }

    /// All available channels.
    pub fn all() -> &'static [SnapChannel] {
        &[Self::Stable, Self::Candidate, Self::Beta, Self::Edge]
    }

    /// Stable channel only.
    pub fn stable() -> &'static [SnapChannel] {
        &[Self::Stable]
    }

    /// Development channels (candidate, beta, edge).
    pub fn dev() -> &'static [SnapChannel] {
        &[Self::Candidate, Self::Beta, Self::Edge]
    }

    /// Release channels (stable, candidate).
    pub fn release() -> &'static [SnapChannel] {
        &[Self::Stable, Self::Candidate]
    }
}

/// Snap package index fetcher with configurable channels.
pub struct Snap {
    channels: Vec<SnapChannel>,
}

impl Snap {
    /// Snapcraft API base URL.
    const API_BASE: &'static str = "https://api.snapcraft.io/v2/snaps";

    /// Create a fetcher with all channels.
    pub fn all() -> Self {
        Self {
            channels: SnapChannel::all().to_vec(),
        }
    }

    /// Create a fetcher with stable channel only.
    pub fn stable() -> Self {
        Self {
            channels: SnapChannel::stable().to_vec(),
        }
    }

    /// Create a fetcher with development channels.
    pub fn dev() -> Self {
        Self {
            channels: SnapChannel::dev().to_vec(),
        }
    }

    /// Create a fetcher with release channels (stable + candidate).
    pub fn release() -> Self {
        Self {
            channels: SnapChannel::release().to_vec(),
        }
    }

    /// Create a fetcher with custom channel selection.
    pub fn with_channels(channels: &[SnapChannel]) -> Self {
        Self {
            channels: channels.to_vec(),
        }
    }

    /// Fetch snap info from API.
    fn fetch_snap_info(name: &str) -> Result<serde_json::Value, IndexError> {
        let url = format!("{}/info/{}", Self::API_BASE, name);
        let response: serde_json::Value = ureq::get(&url)
            .set("Snap-Device-Series", "16")
            .call()
            .map_err(|_| IndexError::NotFound(name.to_string()))?
            .into_json()?;
        Ok(response)
    }

    /// Extract version from channel map for a specific channel.
    fn get_channel_version(
        channel_map: &[serde_json::Value],
        channel: SnapChannel,
    ) -> Option<(String, Option<String>)> {
        channel_map
            .iter()
            .find(|ch| ch["channel"]["risk"].as_str() == Some(channel.risk()))
            .map(|ch| {
                (
                    ch["version"].as_str().unwrap_or("unknown").to_string(),
                    ch["released-at"].as_str().map(String::from),
                )
            })
    }
}

impl PackageIndex for Snap {
    fn ecosystem(&self) -> &'static str {
        "snap"
    }

    fn display_name(&self) -> &'static str {
        "Snap (Snapcraft)"
    }

    fn fetch(&self, name: &str) -> Result<PackageMeta, IndexError> {
        let response = Self::fetch_snap_info(name)?;
        let snap = &response["snap"];

        // Get the channel map
        let channel_map = response["channel-map"].as_array();

        // Find version from first configured channel that has a release
        let (version, channel_name) = channel_map
            .and_then(|channels| {
                for ch in &self.channels {
                    if let Some((ver, _)) = Self::get_channel_version(channels, *ch) {
                        return Some((ver, ch.name()));
                    }
                }
                None
            })
            .unwrap_or_else(|| {
                (
                    snap["version"].as_str().unwrap_or("unknown").to_string(),
                    "unknown",
                )
            });

        // Extract categories as keywords
        let keywords: Vec<String> = snap["categories"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c["name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Get publisher info
        let maintainers: Vec<String> = snap["publisher"]["display-name"]
            .as_str()
            .or(snap["publisher"]["username"].as_str())
            .map(|p| vec![p.to_string()])
            .unwrap_or_default();

        let mut extra = HashMap::new();
        extra.insert(
            "source_repo".to_string(),
            serde_json::Value::String(channel_name.to_string()),
        );

        Ok(PackageMeta {
            name: snap["name"].as_str().unwrap_or(name).to_string(),
            version,
            description: snap["summary"]
                .as_str()
                .or(snap["description"].as_str())
                .map(String::from),
            homepage: snap["website"].as_str().map(String::from),
            repository: snap["contact"].as_str().and_then(|c| {
                if c.contains("github.com") || c.contains("gitlab.com") {
                    Some(c.to_string())
                } else {
                    None
                }
            }),
            license: snap["license"].as_str().map(String::from),
            binaries: Vec::new(),
            keywords,
            maintainers,
            published: None,
            downloads: None,
            archive_url: None,
            checksum: None,
            extra,
        })
    }

    fn fetch_versions(&self, name: &str) -> Result<Vec<VersionMeta>, IndexError> {
        let response = Self::fetch_snap_info(name)?;

        let channels = response["channel-map"]
            .as_array()
            .ok_or_else(|| IndexError::NotFound(name.to_string()))?;

        // Filter to configured channels and collect unique versions
        let channel_risks: HashSet<_> = self.channels.iter().map(|c| c.risk()).collect();
        let mut seen = HashSet::new();

        let versions: Vec<VersionMeta> = channels
            .iter()
            .filter(|ch| {
                ch["channel"]["risk"]
                    .as_str()
                    .map(|r| channel_risks.contains(r))
                    .unwrap_or(false)
            })
            .filter_map(|ch| {
                let version = ch["version"].as_str()?;
                let risk = ch["channel"]["risk"].as_str().unwrap_or("unknown");
                let key = format!("{}-{}", version, risk);
                if seen.insert(key) {
                    Some(VersionMeta {
                        version: format!("{} ({})", version, risk),
                        released: ch["released-at"].as_str().map(String::from),
                        yanked: false,
                    })
                } else {
                    None
                }
            })
            .collect();

        if versions.is_empty() {
            return Err(IndexError::NotFound(name.to_string()));
        }

        Ok(versions)
    }

    fn search(&self, query: &str) -> Result<Vec<PackageMeta>, IndexError> {
        let url = format!("{}/find?q={}", Self::API_BASE, query);
        let response: serde_json::Value = ureq::get(&url)
            .set("Snap-Device-Series", "16")
            .call()?
            .into_json()?;

        let results = response["results"]
            .as_array()
            .ok_or_else(|| IndexError::Parse("Invalid search response".into()))?;

        Ok(results
            .iter()
            .filter_map(|result| {
                let snap = &result["snap"];
                let mut extra = HashMap::new();
                extra.insert(
                    "source_repo".to_string(),
                    serde_json::Value::String("stable".to_string()),
                );

                Some(PackageMeta {
                    name: snap["name"].as_str()?.to_string(),
                    version: result["version"].as_str().unwrap_or("unknown").to_string(),
                    description: snap["summary"].as_str().map(String::from),
                    homepage: snap["website"].as_str().map(String::from),
                    repository: None,
                    license: snap["license"].as_str().map(String::from),
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
}
