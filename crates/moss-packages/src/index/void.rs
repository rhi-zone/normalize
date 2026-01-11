//! Void Linux package index fetcher (xbps).
//!
//! Fetches package metadata from Void Linux repositories.
//!
//! ## API Strategy
//! - **fetch**: Searches cached `repo-default.voidlinux.org/.../x86_64-repodata` (zstd tar + XML plist)
//! - **fetch_versions**: Same, single version per package
//! - **search**: Filters cached repodata
//! - **fetch_all**: Full repodata (cached 1 hour, ~20MB uncompressed)

use super::{IndexError, PackageIndex, PackageMeta, VersionMeta};
use crate::cache;
use std::io::Read;
use std::time::Duration;

/// Cache TTL for Void package index (1 hour).
const CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// Void Linux package index fetcher.
pub struct Void;

impl Void {
    /// Void Linux repository URL.
    const REPO_URL: &'static str = "https://repo-default.voidlinux.org/current/x86_64-repodata";

    /// Parse plist XML into packages.
    fn parse_plist(xml: &str) -> Result<Vec<PackageMeta>, IndexError> {
        // Parse the XML plist manually since it has a specific structure
        let mut packages = Vec::new();
        let mut current_name: Option<String> = None;
        let mut in_package = false;
        let mut current_field: Option<String> = None;

        // Simple state machine parser for plist XML
        let mut version = String::new();
        let mut homepage = String::new();
        let mut description = String::new();
        let mut license = String::new();
        let mut maintainer = String::new();

        for line in xml.lines() {
            let line = line.trim();

            if line.starts_with("<key>") && line.ends_with("</key>") {
                let key = &line[5..line.len() - 6];
                if !in_package {
                    // This is a package name
                    current_name = Some(key.to_string());
                    in_package = false; // Will be set true when we see <dict>
                    version.clear();
                    homepage.clear();
                    description.clear();
                    license.clear();
                    maintainer.clear();
                } else {
                    current_field = Some(key.to_string());
                }
            } else if line == "<dict>" && current_name.is_some() && !in_package {
                in_package = true;
            } else if line == "</dict>" && in_package {
                // End of package dict
                if let Some(name) = current_name.take() {
                    // Extract version from pkgver (e.g., "ripgrep-15.1.0_1")
                    let (pkg_name, ver) = if version.contains('-') {
                        let parts: Vec<&str> = version.rsplitn(2, '-').collect();
                        if parts.len() == 2 {
                            (parts[1].to_string(), parts[0].to_string())
                        } else {
                            (name.clone(), version.clone())
                        }
                    } else {
                        (name.clone(), version.clone())
                    };

                    packages.push(PackageMeta {
                        name: pkg_name,
                        version: ver,
                        description: if description.is_empty() {
                            None
                        } else {
                            Some(description.clone())
                        },
                        homepage: if homepage.is_empty() {
                            None
                        } else {
                            Some(homepage.clone())
                        },
                        repository: Some("https://github.com/void-linux/void-packages".to_string()),
                        license: if license.is_empty() {
                            None
                        } else {
                            Some(license.clone())
                        },
                        maintainers: if maintainer.is_empty() {
                            Vec::new()
                        } else {
                            vec![maintainer.clone()]
                        },
                        binaries: Vec::new(),
                        ..Default::default()
                    });
                }
                in_package = false;
            } else if line.starts_with("<string>") && line.ends_with("</string>") {
                let value = &line[8..line.len() - 9];
                if let Some(field) = &current_field {
                    match field.as_str() {
                        "pkgver" => version = value.to_string(),
                        "homepage" => homepage = value.to_string(),
                        "short_desc" => description = value.to_string(),
                        "license" => license = value.to_string(),
                        "maintainer" => maintainer = value.to_string(),
                        _ => {}
                    }
                }
                current_field = None;
            }
        }

        Ok(packages)
    }

    /// Load and parse the package index.
    fn load_packages() -> Result<Vec<PackageMeta>, IndexError> {
        let (data, _was_cached) =
            cache::fetch_with_cache("void", "repodata", Self::REPO_URL, CACHE_TTL)
                .map_err(IndexError::Network)?;

        // Decompress zstd
        let decompressed = zstd::decode_all(std::io::Cursor::new(&data))
            .map_err(|e| IndexError::Decompress(e.to_string()))?;

        // Extract tar
        let mut archive = tar::Archive::new(std::io::Cursor::new(decompressed));

        for entry in archive.entries().map_err(|e| IndexError::Io(e))? {
            let mut entry = entry.map_err(|e| IndexError::Io(e))?;
            let path = entry.path().map_err(|e| IndexError::Io(e))?;

            if path.to_string_lossy() == "index.plist" {
                let mut xml = String::new();
                entry
                    .read_to_string(&mut xml)
                    .map_err(|e| IndexError::Io(e))?;
                return Self::parse_plist(&xml);
            }
        }

        Err(IndexError::Parse("index.plist not found in archive".into()))
    }
}

impl PackageIndex for Void {
    fn ecosystem(&self) -> &'static str {
        "void"
    }

    fn display_name(&self) -> &'static str {
        "Void Linux (xbps)"
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
