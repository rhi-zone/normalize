//! Local cache for package indices (offline support).

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// HTTP cache metadata for index files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct IndexMeta {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cached_at: u64, // Unix timestamp
    pub url: String,
}

/// Get base cache directory: ~/.cache/moss
fn cache_base() -> Option<PathBuf> {
    let base = if let Ok(cache) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(cache)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache")
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home).join(".cache")
    } else {
        return None;
    };
    Some(base.join("moss"))
}

/// Get index cache directory: ~/.cache/moss/indices
fn index_cache_dir() -> Option<PathBuf> {
    Some(cache_base()?.join("indices"))
}

/// Generate a safe cache key from a URL.
#[allow(dead_code)]
pub fn index_cache_key(url: &str) -> String {
    // Use a simple hash-like approach: take the URL and make it filesystem-safe
    url.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect()
}

/// Get paths for cached index data and metadata.
fn index_paths(ecosystem: &str, name: &str) -> Option<(PathBuf, PathBuf)> {
    let dir = index_cache_dir()?.join(ecosystem);
    let data_path = dir.join(format!("{}.data", name));
    let meta_path = dir.join(format!("{}.meta.json", name));
    Some((data_path, meta_path))
}

/// Read index metadata (for staleness check).
pub fn read_index_meta(ecosystem: &str, name: &str) -> Option<IndexMeta> {
    let (_, meta_path) = index_paths(ecosystem, name)?;
    let content = fs::read_to_string(&meta_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Read cached index data.
pub fn read_index(ecosystem: &str, name: &str) -> Option<Vec<u8>> {
    let (data_path, _) = index_paths(ecosystem, name)?;
    fs::read(&data_path).ok()
}

/// Read cached index if not expired.
pub fn read_index_if_fresh(ecosystem: &str, name: &str, max_age: Duration) -> Option<Vec<u8>> {
    let meta = read_index_meta(ecosystem, name)?;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if now - meta.cached_at > max_age.as_secs() {
        return None; // Expired
    }

    read_index(ecosystem, name)
}

/// Write index data and metadata to cache.
pub fn write_index(
    ecosystem: &str,
    name: &str,
    data: &[u8],
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) {
    let Some((data_path, meta_path)) = index_paths(ecosystem, name) else {
        return;
    };

    // Create directory if needed
    if let Some(parent) = data_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Write data
    if fs::write(&data_path, data).is_err() {
        return;
    }

    // Write metadata
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let meta = IndexMeta {
        etag: etag.map(String::from),
        last_modified: last_modified.map(String::from),
        cached_at: now,
        url: url.to_string(),
    };

    if let Ok(json) = serde_json::to_string_pretty(&meta) {
        let _ = fs::write(&meta_path, json);
    }
}

/// Fetch URL with cache support using conditional requests.
/// Returns (data, was_cached) tuple.
pub fn fetch_with_cache(
    ecosystem: &str,
    name: &str,
    url: &str,
    max_age: Duration,
) -> Result<(Vec<u8>, bool), String> {
    // Check if we have fresh cached data
    if let Some(data) = read_index_if_fresh(ecosystem, name, max_age) {
        return Ok((data, true));
    }

    // Check for stale cache to use conditional request
    let meta = read_index_meta(ecosystem, name);

    // Build request with conditional headers
    let mut request = ureq::get(url);

    if let Some(ref m) = meta {
        if let Some(ref etag) = m.etag {
            request = request.set("If-None-Match", etag);
        }
        if let Some(ref lm) = m.last_modified {
            request = request.set("If-Modified-Since", lm);
        }
    }

    let response = request.call().map_err(|e| e.to_string())?;

    // 304 Not Modified - use cached data
    if response.status() == 304 {
        if let Some(data) = read_index(ecosystem, name) {
            // Update cached_at timestamp
            if let Some(m) = meta {
                write_index(
                    ecosystem,
                    name,
                    &data,
                    url,
                    m.etag.as_deref(),
                    m.last_modified.as_deref(),
                );
            }
            return Ok((data, true));
        }
    }

    // Get response headers for caching
    let etag = response.header("ETag").map(String::from);
    let last_modified = response.header("Last-Modified").map(String::from);

    // Read response body
    let mut data = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;

    // Cache the response
    write_index(
        ecosystem,
        name,
        &data,
        url,
        etag.as_deref(),
        last_modified.as_deref(),
    );

    Ok((data, false))
}
