//! Download and extract package source archives into the local cache.
//!
//! Many ecosystems expose a package's source as a downloadable archive (a `.zip`
//! for Go module proxies, a `.tar.gz` sdist for Python, etc.). This module fetches
//! such an archive over HTTP and extracts it under
//! `~/.cache/normalize/sources/{ecosystem}/{package}/{version}/`, so that
//! tree-sitter-based doc extraction (see [`crate::doc_tree`]) can operate on the
//! on-disk source tree.
//!
//! Extraction is staged into a [`tempfile::TempDir`] and atomically renamed into
//! place, so a partial or interrupted extraction never leaves a directory that
//! looks complete. If the target directory already exists, the download is skipped
//! (cache hit) and the existing path is returned.

use crate::PackageError;
use crate::cache::cache_base;
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// The compression/container format of a source archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    /// A ZIP archive (e.g. Go module proxy `.zip`).
    Zip,
    /// A gzip-compressed tarball (e.g. Python sdist `.tar.gz`).
    TarGz,
}

/// Download and extract a package's source archive into the local cache.
///
/// Returns the path to the extracted source directory
/// (`~/.cache/normalize/sources/{ecosystem}/{package}/{version}/`).
///
/// If that directory already exists, no download is performed (cache hit).
/// Otherwise the archive at `url` is downloaded via [`crate::http::get_bytes`],
/// extracted into a temporary directory, and atomically renamed into place.
pub fn fetch_and_extract(
    ecosystem: &str,
    package: &str,
    version: &str,
    url: &str,
    kind: ArchiveKind,
) -> Result<PathBuf, PackageError> {
    let target = source_cache_dir(ecosystem, package, version).ok_or_else(|| {
        PackageError::ParseError("could not resolve cache directory (no HOME?)".to_string())
    })?;

    // Cache hit: already extracted.
    if target.is_dir() {
        return Ok(target);
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PackageError::ParseError(format!("failed to create cache dir: {}", e)))?;
    }

    let bytes = crate::http::get_bytes(url)?;

    // Stage extraction in a temp dir adjacent to the target, then atomically rename.
    let staging_parent = target.parent().unwrap_or_else(|| Path::new("."));
    let staging = tempfile::TempDir::new_in(staging_parent)
        .map_err(|e| PackageError::ParseError(format!("failed to create temp dir: {}", e)))?;

    match kind {
        ArchiveKind::Zip => extract_zip(&bytes, staging.path())?,
        ArchiveKind::TarGz => extract_tar_gz(&bytes, staging.path())?,
    }

    // Atomic rename into place. If another process won the race and the target now
    // exists, treat it as a cache hit.
    let staged_path = staging.keep();
    match std::fs::rename(&staged_path, &target) {
        Ok(()) => Ok(target),
        Err(_) if target.is_dir() => {
            let _ = std::fs::remove_dir_all(&staged_path);
            Ok(target)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staged_path);
            Err(PackageError::ParseError(format!(
                "failed to move extracted source into place: {}",
                e
            )))
        }
    }
}

/// Compute the cache directory for a package's extracted source.
fn source_cache_dir(ecosystem: &str, package: &str, version: &str) -> Option<PathBuf> {
    let safe_pkg = package.replace(['/', ':'], "_");
    let safe_ver = version.replace(['/', ':'], "_");
    Some(
        cache_base()?
            .join("sources")
            .join(ecosystem)
            .join(safe_pkg)
            .join(safe_ver),
    )
}

/// Extract a ZIP archive's bytes into `dest`.
fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), PackageError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| PackageError::ParseError(format!("invalid zip archive: {}", e)))?;
    archive
        .extract(dest)
        .map_err(|e| PackageError::ParseError(format!("failed to extract zip: {}", e)))
}

/// Extract a gzip-compressed tarball's bytes into `dest`.
fn extract_tar_gz(bytes: &[u8], dest: &Path) -> Result<(), PackageError> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(dest)
        .map_err(|e| PackageError::ParseError(format!("failed to extract tar.gz: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a tiny `.tar.gz` in memory containing `path -> contents` entries.
    fn build_tar_gz(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (path, contents) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(contents.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, path, contents.as_bytes())
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&tar_bytes).unwrap();
        gz.finish().unwrap()
    }

    /// Build a tiny `.zip` in memory containing `path -> contents` entries.
    fn build_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            for (path, contents) in entries {
                writer.start_file(*path, opts).unwrap();
                writer.write_all(contents.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
        }
        buf
    }

    #[test]
    fn extract_tar_gz_lands_files() {
        let bytes = build_tar_gz(&[("pkg/foo.go", "package pkg\n")]);
        let tmp = tempfile::TempDir::new().unwrap();
        extract_tar_gz(&bytes, tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path().join("pkg/foo.go")).unwrap();
        assert_eq!(content, "package pkg\n");
    }

    #[test]
    fn extract_zip_lands_files() {
        let bytes = build_zip(&[("pkg/bar.py", "x = 1\n")]);
        let tmp = tempfile::TempDir::new().unwrap();
        extract_zip(&bytes, tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path().join("pkg/bar.py")).unwrap();
        assert_eq!(content, "x = 1\n");
    }

    #[test]
    fn source_cache_dir_sanitizes() {
        // Set a known cache base so the test is deterministic.
        let tmp = tempfile::TempDir::new().unwrap();
        // SAFETY: single-threaded test; restored not needed as TempDir is scoped.
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", tmp.path());
        }
        let dir = source_cache_dir("go", "github.com/foo/bar", "v1.2.3").unwrap();
        assert!(dir.ends_with("normalize/sources/go/github.com_foo_bar/v1.2.3"));
        unsafe {
            std::env::remove_var("XDG_CACHE_HOME");
        }
    }
}
