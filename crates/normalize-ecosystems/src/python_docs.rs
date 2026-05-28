//! Python symbol documentation: local (site-packages) + remote (PyPI sdist).
//!
//! Two [`crate`] doc providers for Python:
//!
//! - [`PythonLocalDocsExtractor`] resolves a package to its on-disk source in the
//!   project's `site-packages` (or stdlib) via [`normalize_local_deps`], then runs
//!   the shared tree-sitter extractor ([`crate::doc_tree::extract_from_source_tree`]).
//!   The installed version is recovered from a matching `{name}-{version}.dist-info`
//!   directory when available (PyPI/pip layout); otherwise it is left empty.
//! - [`PythonRemoteDocsFetcher`] resolves the sdist URL from the PyPI JSON API,
//!   downloads and extracts the source archive ([`crate::source_archive`]), and runs
//!   the same tree-sitter extractor over the extracted `{pkg}-{version}/` tree.
//!
//! Both report `doc_format: DocFormat::PlainText` — we extract the raw docstring text
//! and do not interpret RST / Google / NumPy docstring conventions.

use crate::{
    DocsError, LocalDocsExtractor, PackageError, RemoteDocsFetcher, doc_tree, source_archive,
    source_archive::ArchiveKind, symbol_docs::SymbolDoc,
};
use normalize_local_deps::LocalDeps;
use normalize_local_deps::python::{PythonDeps, find_python_site_packages};
use std::path::{Path, PathBuf};

/// Tree-sitter grammar name for Python.
const PYTHON_GRAMMAR: &str = "python";

/// PyPI JSON API base URL.
const PYPI_BASE: &str = "https://pypi.org/pypi";

// ── local extractor ────────────────────────────────────────────────────────

/// Local docs extractor for Python packages.
///
/// Resolves a top-level package/module name to its on-disk source in the project's
/// `site-packages` (or stdlib) directory, then extracts the requested symbol's
/// docstring from the source. A package resolves to a directory (`requests/`) or a
/// single-file module (`six.py`); single-file modules are handled by pointing the
/// shared file-walk at the module's parent directory.
pub struct PythonLocalDocsExtractor {
    /// Project root — used to locate the active venv / site-packages.
    project_root: PathBuf,
}

impl PythonLocalDocsExtractor {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }
}

impl LocalDocsExtractor for PythonLocalDocsExtractor {
    fn extract_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<SymbolDoc, DocsError> {
        // `package` is the top-level import name (e.g. "requests", "six").
        // PythonDeps resolves stdlib (lib/pythonX.Y) and site-packages imports.
        let resolved = PythonDeps
            .resolve_external_import(package, &self.project_root)
            .ok_or_else(|| {
                DocsError::NotFound(format!(
                    "package '{}' not found in site-packages or stdlib \
                     (install it into the project's venv, or check the venv is detected)",
                    package
                ))
            })?;

        // `resolved.path` is either a package directory or a single-file `.py`
        // module. `extract_from_source_tree` walks a directory tree, so for a
        // single file we point it at the parent directory (the walk finds the file).
        let search_dir: &Path = if resolved.path.is_dir() {
            &resolved.path
        } else {
            resolved.path.parent().ok_or_else(|| {
                DocsError::NotFound(format!("resolved module '{}' has no parent dir", package))
            })?
        };

        // Version: explicit arg wins; otherwise read it from a `{name}-{ver}.dist-info`
        // directory in site-packages (pip/PyPI layout). stdlib modules have no
        // dist-info — leave the version empty in that case.
        let resolved_version = version.map(str::to_string).or_else(|| {
            find_python_site_packages(&self.project_root)
                .and_then(|sp| dist_info_version(&sp, package))
        });

        doc_tree::extract_from_source_tree(
            search_dir,
            PYTHON_GRAMMAR,
            package,
            symbol_path,
            resolved_version.as_deref().unwrap_or_default(),
        )
    }
}

/// Find the installed version of `package` from a `{name}-{version}.dist-info`
/// directory under `site_packages`.
///
/// PyPI/pip records each installed distribution as `{Name}-{Version}.dist-info/`.
/// Names are matched per PEP 503 normalization (lowercase, `-`/`.`/`_` collapsed to
/// `_`), since the import name and the distribution name can differ in casing and
/// separator. Returns `None` if no matching dist-info directory is found.
fn dist_info_version(site_packages: &Path, package: &str) -> Option<String> {
    let target = normalize_dist_name(package);
    let entries = std::fs::read_dir(site_packages).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(stem) = name.strip_suffix(".dist-info") else {
            continue;
        };
        // stem is `{Name}-{Version}`; split on the last '-'.
        if let Some((dist_name, version)) = stem.rsplit_once('-')
            && normalize_dist_name(dist_name) == target
        {
            return Some(version.to_string());
        }
    }
    None
}

/// Normalize a distribution / import name for comparison (PEP 503-ish).
fn normalize_dist_name(name: &str) -> String {
    name.to_lowercase().replace(['-', '.'], "_")
}

// ── remote fetcher ─────────────────────────────────────────────────────────

/// Remote docs fetcher for Python: downloads the sdist source archive from PyPI.
pub struct PythonRemoteDocsFetcher;

impl RemoteDocsFetcher for PythonRemoteDocsFetcher {
    fn fetch_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<SymbolDoc, DocsError> {
        // Resolve the version: explicit, or the latest from the project-level JSON.
        let resolved_version = match version {
            Some(v) => v.to_string(),
            None => latest_version(package)?,
        };

        // Locate the sdist artifact for this exact version.
        let (url, kind) = pypi_sdist_url(package, &resolved_version)?.ok_or_else(|| {
            DocsError::NotFound(format!(
                "no sdist (source distribution) found for '{}' {} on PyPI \
                 (only wheels published?)",
                package, resolved_version
            ))
        })?;

        // Download + extract. sdists expand to a `{pkg}-{version}/` top dir (sometimes
        // with a `src/` layout); the recursive walk in extract_from_source_tree handles
        // the nesting, so we point it at the extracted root.
        let dir =
            source_archive::fetch_and_extract("python", package, &resolved_version, &url, kind)?;

        doc_tree::extract_from_source_tree(
            &dir,
            PYTHON_GRAMMAR,
            package,
            symbol_path,
            &resolved_version,
        )
    }
}

/// Resolve the latest published version of `package` from the PyPI JSON API.
fn latest_version(package: &str) -> Result<String, DocsError> {
    let url = format!("{}/{}/json", PYPI_BASE, package);
    let body = crate::http::get(&url)?;
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| DocsError::ParseError(format!("invalid PyPI JSON: {}", e)))?;
    v.get("info")
        .and_then(|info| info.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| DocsError::NotFound(format!("no version found for package '{}'", package)))
}

/// Resolve the sdist download URL (and archive kind) for an exact `package` version.
///
/// Hits `GET {PYPI_BASE}/{package}/{version}/json` and scans the `urls[]` array for
/// the entry whose `packagetype == "sdist"`. The [`ArchiveKind`] is chosen from the
/// filename / URL extension — `.zip` is rare but valid; everything else is treated as
/// `.tar.gz`. Returns `Ok(None)` when the version exists but publishes no sdist (e.g.
/// wheel-only releases).
pub fn pypi_sdist_url(
    package: &str,
    version: &str,
) -> Result<Option<(String, ArchiveKind)>, PackageError> {
    let url = format!("{}/{}/{}/json", PYPI_BASE, package, version);
    let body = crate::http::get(&url)?;
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| PackageError::ParseError(format!("invalid PyPI JSON: {}", e)))?;

    let Some(urls) = v.get("urls").and_then(|u| u.as_array()) else {
        return Ok(None);
    };

    for entry in urls {
        if entry.get("packagetype").and_then(|t| t.as_str()) != Some("sdist") {
            continue;
        }
        let Some(download_url) = entry.get("url").and_then(|u| u.as_str()) else {
            continue;
        };
        let filename = entry
            .get("filename")
            .and_then(|f| f.as_str())
            .unwrap_or(download_url);
        return Ok(Some((download_url.to_string(), archive_kind_for(filename))));
    }

    Ok(None)
}

/// Pick the [`ArchiveKind`] from a filename / URL. `.zip` → `Zip`, else `TarGz`.
fn archive_kind_for(name: &str) -> ArchiveKind {
    if name.to_lowercase().ends_with(".zip") {
        ArchiveKind::Zip
    } else {
        ArchiveKind::TarGz
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol_docs::DocFormat;

    #[test]
    fn archive_kind_picks_zip_for_zip_extension() {
        assert_eq!(archive_kind_for("foo-1.0.zip"), ArchiveKind::Zip);
        assert_eq!(archive_kind_for("FOO-1.0.ZIP"), ArchiveKind::Zip);
        assert_eq!(archive_kind_for("foo-1.0.tar.gz"), ArchiveKind::TarGz);
        assert_eq!(archive_kind_for("foo-1.0.tgz"), ArchiveKind::TarGz);
    }

    #[test]
    fn normalize_dist_name_collapses_separators() {
        assert_eq!(normalize_dist_name("Foo-Bar"), "foo_bar");
        assert_eq!(normalize_dist_name("foo.bar"), "foo_bar");
        assert_eq!(normalize_dist_name("requests"), "requests");
    }

    #[test]
    fn dist_info_version_reads_matching_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("requests-2.32.0.dist-info")).unwrap();
        std::fs::create_dir(tmp.path().join("Other_Pkg-1.2.3.dist-info")).unwrap();
        assert_eq!(
            dist_info_version(tmp.path(), "requests").as_deref(),
            Some("2.32.0")
        );
        // Casing / separator differences are normalized.
        assert_eq!(
            dist_info_version(tmp.path(), "other-pkg").as_deref(),
            Some("1.2.3")
        );
        assert_eq!(dist_info_version(tmp.path(), "missing"), None);
    }

    /// Offline test: build a tiny Python module source tree and extract a documented
    /// class through the shared source-tree extractor (the final hop the local
    /// extractor delegates to once the source dir is resolved).
    #[test]
    fn local_extractor_reads_doc_from_source_tree() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = r#"class Session:
    """A persistent HTTP session."""

    def get(self, url):
        """Send a GET request."""
        return None
"#;
        std::fs::write(tmp.path().join("sessions.py"), src).unwrap();

        let doc = doc_tree::extract_from_source_tree(
            tmp.path(),
            PYTHON_GRAMMAR,
            "requests",
            "Session",
            "2.32.0",
        )
        .expect("should extract Session");
        assert_eq!(doc.name, "Session");
        assert_eq!(doc.language, "python");
        assert_eq!(doc.package, "requests");
        assert_eq!(doc.version, "2.32.0");
        assert_eq!(doc.doc_format, DocFormat::PlainText);
        assert!(
            doc.signature.as_deref().unwrap().contains("Session"),
            "signature: {:?}",
            doc.signature
        );
        assert!(
            doc.doc_body.contains("persistent HTTP session"),
            "doc_body: {:?}",
            doc.doc_body
        );
    }

    /// Offline test: a single-file module's parent dir is searched correctly.
    #[test]
    fn extracts_function_from_single_file_module() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = r#"def utility(x):
    """Do a useful thing."""
    return x
"#;
        std::fs::write(tmp.path().join("six.py"), src).unwrap();

        let doc =
            doc_tree::extract_from_source_tree(tmp.path(), PYTHON_GRAMMAR, "six", "utility", "")
                .expect("should extract utility");
        assert_eq!(doc.name, "utility");
        assert_eq!(doc.version, "");
        assert!(doc.doc_body.contains("useful thing"));
    }

    /// Network test (PyPI sdist download) — ignored by default.
    #[test]
    #[ignore = "network"]
    fn remote_fetch_requests_session() {
        let doc = PythonRemoteDocsFetcher
            .fetch_docs("requests", "Session", None)
            .expect("should fetch Session from PyPI sdist");
        assert_eq!(doc.name, "Session");
        assert_eq!(doc.language, "python");
        println!("{}", doc.doc_body);
    }
}
