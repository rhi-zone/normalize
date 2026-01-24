//! NuGet package index fetcher (.NET).
//!
//! Fetches package metadata from nuget.org.
//!
//! ## API Strategy
//! - **fetch**: `api.nuget.org/v3/registration5-gz-semver2/{name}/index.json` - NuGet v3 API
//! - **fetch_versions**: Same API, extracts versions from catalog pages
//! - **search**: `api.nuget.org/v3/query?q=`
//! - **fetch_all**: Catalog API for incremental updates (streaming)
//!
//! The NuGet catalog is an append-only log of all package operations, enabling
//! efficient incremental synchronization of package metadata.

use super::{IndexError, PackageIndex, PackageIter, PackageMeta, VersionMeta};
use std::collections::HashSet;

/// NuGet package index fetcher.
pub struct Nuget;

impl Nuget {
    /// NuGet API v3.
    const NUGET_API: &'static str = "https://api.nuget.org/v3";
    /// NuGet Catalog API.
    const CATALOG_INDEX: &'static str = "https://api.nuget.org/v3/catalog0/index.json";
}

impl PackageIndex for Nuget {
    fn ecosystem(&self) -> &'static str {
        "nuget"
    }

    fn display_name(&self) -> &'static str {
        "NuGet (.NET)"
    }

    fn fetch(&self, name: &str) -> Result<PackageMeta, IndexError> {
        let name_lower = name.to_lowercase();
        let url = format!(
            "{}/registration5-semver1/{}/index.json",
            Self::NUGET_API,
            name_lower
        );
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        // Get the latest catalog entry
        let items = response["items"]
            .as_array()
            .ok_or_else(|| IndexError::Parse("missing items".into()))?;

        let latest_page = items
            .last()
            .ok_or_else(|| IndexError::NotFound(name.to_string()))?;

        let page_items = latest_page["items"]
            .as_array()
            .ok_or_else(|| IndexError::Parse("missing page items".into()))?;

        let latest = page_items
            .last()
            .ok_or_else(|| IndexError::NotFound(name.to_string()))?;

        let catalog = &latest["catalogEntry"];

        Ok(PackageMeta {
            name: catalog["id"].as_str().unwrap_or(name).to_string(),
            version: catalog["version"].as_str().unwrap_or("unknown").to_string(),
            description: catalog["description"].as_str().map(String::from),
            homepage: catalog["projectUrl"].as_str().map(String::from),
            repository: catalog["repository"]
                .as_str()
                .or_else(|| {
                    // Try to extract from projectUrl if it's a GitHub link
                    catalog["projectUrl"]
                        .as_str()
                        .filter(|u| u.contains("github.com"))
                })
                .map(String::from),
            license: catalog["licenseExpression"]
                .as_str()
                .or_else(|| catalog["licenseUrl"].as_str())
                .map(String::from),
            binaries: Vec::new(),
            keywords: catalog["tags"]
                .as_str()
                .map(|t| t.split_whitespace().map(String::from).collect())
                .unwrap_or_default(),
            maintainers: catalog["authors"]
                .as_str()
                .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
            published: catalog["published"].as_str().map(String::from),
            downloads: None, // Not in registration API
            archive_url: latest["packageContent"].as_str().map(String::from),
            checksum: None, // Not in registration API
            extra: Default::default(),
        })
    }

    fn fetch_versions(&self, name: &str) -> Result<Vec<VersionMeta>, IndexError> {
        let name_lower = name.to_lowercase();
        let url = format!(
            "{}/registration5-semver1/{}/index.json",
            Self::NUGET_API,
            name_lower
        );
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        let items = response["items"]
            .as_array()
            .ok_or_else(|| IndexError::Parse("missing items".into()))?;

        let mut versions = Vec::new();

        for page in items {
            if let Some(page_items) = page["items"].as_array() {
                for item in page_items {
                    let catalog = &item["catalogEntry"];
                    if let Some(version) = catalog["version"].as_str() {
                        versions.push(VersionMeta {
                            version: version.to_string(),
                            released: catalog["published"].as_str().map(String::from),
                            yanked: catalog["listed"].as_bool().map(|l| !l).unwrap_or(false),
                        });
                    }
                }
            }
        }

        Ok(versions)
    }

    fn search(&self, query: &str) -> Result<Vec<PackageMeta>, IndexError> {
        let url = format!(
            "{}-flatcontainer/query?q={}&take=50",
            Self::NUGET_API,
            query
        );
        let response: serde_json::Value = ureq::get(&url).call()?.into_json()?;

        let data = response["data"]
            .as_array()
            .ok_or_else(|| IndexError::Parse("missing data".into()))?;

        Ok(data
            .iter()
            .filter_map(|pkg| {
                Some(PackageMeta {
                    name: pkg["id"].as_str()?.to_string(),
                    version: pkg["version"].as_str().unwrap_or("unknown").to_string(),
                    description: pkg["description"].as_str().map(String::from),
                    homepage: pkg["projectUrl"].as_str().map(String::from),
                    repository: None,
                    license: pkg["licenseUrl"].as_str().map(String::from),
                    binaries: Vec::new(),
                    keywords: pkg["tags"]
                        .as_array()
                        .map(|t| {
                            t.iter()
                                .filter_map(|s| s.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    maintainers: pkg["authors"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|s| s.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    published: None,
                    downloads: pkg["totalDownloads"].as_u64(),
                    archive_url: None,
                    checksum: None,
                    extra: Default::default(),
                })
            })
            .collect())
    }

    fn supports_fetch_all(&self) -> bool {
        true
    }

    fn fetch_all(&self) -> Result<Vec<PackageMeta>, IndexError> {
        self.iter_all()?.collect()
    }

    fn iter_all(&self) -> Result<PackageIter<'_>, IndexError> {
        // Fetch catalog index
        let response: serde_json::Value = ureq::get(Self::CATALOG_INDEX).call()?.into_json()?;

        let items = response["items"]
            .as_array()
            .ok_or_else(|| IndexError::Parse("missing catalog items".into()))?;

        // Collect page URLs
        let page_urls: Vec<String> = items
            .iter()
            .filter_map(|item| item["@id"].as_str().map(String::from))
            .collect();

        Ok(Box::new(NuGetCatalogIter {
            page_urls,
            current_page_idx: 0,
            current_packages: Vec::new(),
            seen: HashSet::new(),
        }))
    }
}

/// Iterator over NuGet catalog pages.
struct NuGetCatalogIter {
    page_urls: Vec<String>,
    current_page_idx: usize,
    current_packages: Vec<PackageMeta>,
    /// Track seen package IDs to deduplicate (catalog has multiple versions/updates per package)
    seen: HashSet<String>,
}

impl Iterator for NuGetCatalogIter {
    type Item = Result<PackageMeta, IndexError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Return next package from current page
            if let Some(pkg) = self.current_packages.pop() {
                return Some(Ok(pkg));
            }

            // Load next page
            if self.current_page_idx >= self.page_urls.len() {
                return None;
            }

            let page_url = &self.page_urls[self.current_page_idx];
            self.current_page_idx += 1;

            match ureq::get(page_url).call() {
                Ok(response) => match response.into_json::<serde_json::Value>() {
                    Ok(page) => {
                        if let Some(items) = page["items"].as_array() {
                            for item in items {
                                // Only process PackageDetails, skip PackageDelete
                                let item_type = item["@type"].as_str().unwrap_or("");
                                if !item_type.contains("PackageDetails") {
                                    continue;
                                }

                                let id = match item["nuget:id"].as_str() {
                                    Some(id) => id.to_lowercase(),
                                    None => continue,
                                };

                                // Skip if we've already seen this package
                                if self.seen.contains(&id) {
                                    continue;
                                }
                                self.seen.insert(id.clone());

                                let pkg = PackageMeta {
                                    name: item["nuget:id"].as_str().unwrap_or(&id).to_string(),
                                    version: item["nuget:version"]
                                        .as_str()
                                        .unwrap_or("unknown")
                                        .to_string(),
                                    description: None, // Not in catalog leaf
                                    homepage: None,
                                    repository: None,
                                    license: None,
                                    binaries: Vec::new(),
                                    keywords: Vec::new(),
                                    maintainers: Vec::new(),
                                    published: item["commitTimeStamp"].as_str().map(String::from),
                                    downloads: None,
                                    archive_url: None,
                                    checksum: None,
                                    extra: Default::default(),
                                };
                                self.current_packages.push(pkg);
                            }
                        }
                    }
                    Err(e) => return Some(Err(IndexError::Parse(e.to_string()))),
                },
                Err(e) => return Some(Err(IndexError::Network(e.to_string()))),
            }
        }
    }
}
