//! Local cache for package info (offline support).

use crate::PackageInfo;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Cache entry with timestamp.
#[derive(serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    info: PackageInfo,
    cached_at: u64, // Unix timestamp
}

/// Get base cache directory: ~/.cache/normalize
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
    Some(base.join("normalize"))
}

/// Get cache directory: ~/.cache/normalize/packages
fn cache_dir() -> Option<PathBuf> {
    Some(cache_base()?.join("packages"))
}

/// Get cache file path for a package.
fn cache_path(ecosystem: &str, package: &str) -> Option<PathBuf> {
    let dir = cache_dir()?;
    // Sanitize package name for filesystem
    let safe_name = package.replace(['/', ':'], "_");
    Some(dir.join(ecosystem).join(format!("{}.json", safe_name)))
}

/// Read from cache if exists and not expired.
pub fn read(ecosystem: &str, package: &str, max_age: Duration) -> Option<PackageInfo> {
    let path = cache_path(ecosystem, package)?;
    let content = fs::read_to_string(&path).ok()?;
    let entry: CacheEntry = serde_json::from_str(&content).ok()?;

    // Check expiry
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if now - entry.cached_at > max_age.as_secs() {
        return None; // Expired
    }

    Some(entry.info)
}

/// Read from cache regardless of age (for offline fallback).
pub fn read_any(ecosystem: &str, package: &str) -> Option<PackageInfo> {
    let path = cache_path(ecosystem, package)?;
    let content = fs::read_to_string(&path).ok()?;
    let entry: CacheEntry = serde_json::from_str(&content).ok()?;
    Some(entry.info)
}

/// Write to cache.
pub fn write(ecosystem: &str, package: &str, info: &PackageInfo) {
    let Some(path) = cache_path(ecosystem, package) else {
        return;
    };

    // Create directory if needed
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let entry = CacheEntry {
        info: info.clone(),
        cached_at: now,
    };

    if let Ok(json) = serde_json::to_string(&entry) {
        let _ = fs::write(&path, json);
    }
}
