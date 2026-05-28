//! Go symbol documentation: local (module cache / SDK) + remote (module proxy).
//!
//! Two [`crate`] doc providers for Go:
//!
//! - [`GoLocalDocsExtractor`] resolves an import path to its on-disk source via the
//!   Go module cache (`$GOMODCACHE`) or the SDK's `src/` tree (stdlib), then runs the
//!   shared tree-sitter extractor ([`crate::doc_tree::extract_from_source_tree`]).
//! - [`GoRemoteDocsFetcher`] downloads the module's source `.zip` from the Go module
//!   proxy (`proxy.golang.org`), extracts it into the source cache
//!   ([`crate::source_archive`]), and runs the same tree-sitter extractor.
//!
//! Both report `doc_format: DocFormat::PlainText` (Go doc comments are plain text).

use crate::{
    DocsError, LocalDocsExtractor, RemoteDocsFetcher, doc_tree, source_archive,
    source_archive::ArchiveKind, symbol_docs::SymbolDoc,
};
use normalize_local_deps::LocalDeps;
use normalize_local_deps::go::GoDeps;
use std::path::{Path, PathBuf};

/// Tree-sitter grammar name for Go.
const GO_GRAMMAR: &str = "go";

/// Go module proxy base URL.
const GO_PROXY: &str = "https://proxy.golang.org";

// ── local extractor ────────────────────────────────────────────────────────

/// Local docs extractor for Go packages.
///
/// Resolves an import path to a source directory in the Go module cache
/// (`$GOMODCACHE`) — or the Go SDK `src/` tree for standard-library packages —
/// then extracts the requested symbol's doc comment from the on-disk source.
pub struct GoLocalDocsExtractor {
    /// Project root (used for import resolution; not strictly required for Go,
    /// since the module cache is global, but kept for interface symmetry).
    project_root: PathBuf,
}

impl GoLocalDocsExtractor {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }
}

impl LocalDocsExtractor for GoLocalDocsExtractor {
    fn extract_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<SymbolDoc, DocsError> {
        // `package` is the import path (e.g. "fmt", "github.com/gin-gonic/gin").
        // GoDeps resolves both stdlib (GOROOT/src) and module-cache imports.
        let resolved = GoDeps
            .resolve_external_import(package, &self.project_root)
            .ok_or_else(|| {
                DocsError::NotFound(format!(
                    "import path '{}' not found in Go module cache or SDK \
                     (run `go mod download` / `go build`, or set GOROOT/GOMODCACHE)",
                    package
                ))
            })?;

        // Derive the version from the cache directory name (`repo@v1.2.3`) when the
        // caller didn't pass one; stdlib dirs have no version suffix.
        let resolved_version = version
            .map(str::to_string)
            .or_else(|| version_from_cache_path(&resolved.path))
            .unwrap_or_default();

        doc_tree::extract_from_source_tree(
            &resolved.path,
            GO_GRAMMAR,
            package,
            symbol_path,
            &resolved_version,
        )
    }
}

/// Extract a `@version` suffix from a Go module-cache directory component.
///
/// Module-cache paths look like `.../github.com/gin-gonic/gin@v1.9.1/...`; we take
/// the version from the last path component that contains an `@`. Returns `None`
/// for stdlib paths (no `@` anywhere).
fn version_from_cache_path(path: &Path) -> Option<String> {
    for comp in path.components().rev() {
        let s = comp.as_os_str().to_string_lossy();
        if let Some((_, ver)) = s.rsplit_once('@') {
            return Some(ver.to_string());
        }
    }
    None
}

// ── remote fetcher ─────────────────────────────────────────────────────────

/// Remote docs fetcher for Go: downloads module source from the Go module proxy.
pub struct GoRemoteDocsFetcher;

impl RemoteDocsFetcher for GoRemoteDocsFetcher {
    fn fetch_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<SymbolDoc, DocsError> {
        // `package` is the module path (e.g. "github.com/gin-gonic/gin").
        // Stdlib has no module proxy source — bail with a clear message.
        if is_stdlib_module(package) {
            return Err(DocsError::NotFound(format!(
                "'{}' is a Go standard-library package; it is not available on the \
                 module proxy. Install a Go SDK so local extraction can read $GOROOT/src.",
                package
            )));
        }

        // Resolve the version: explicit, or `@latest` from the proxy.
        let resolved_version = match version {
            Some(v) => v.to_string(),
            None => resolve_latest_version(package)?,
        };

        // Module-path case-escaping: the Go proxy lowercases uppercase letters by
        // prefixing them with `!` (e.g. `Azure` -> `!azure`). Applies to both the
        // module path and the version in the URL.
        let escaped_module = escape_go_proxy(package);
        let escaped_version = escape_go_proxy(&resolved_version);
        let url = format!("{}/{}/@v/{}.zip", GO_PROXY, escaped_module, escaped_version);

        // Download + extract. The proxy zip contains a `{module}@{version}/` top-level
        // dir; `extract_from_source_tree` walks recursively, so we point it at the
        // extracted root and let the walk find the nested files.
        let dir = source_archive::fetch_and_extract(
            "go",
            package,
            &resolved_version,
            &url,
            ArchiveKind::Zip,
        )?;

        doc_tree::extract_from_source_tree(
            &dir,
            GO_GRAMMAR,
            package,
            symbol_path,
            &resolved_version,
        )
    }
}

/// Resolve the latest version of a Go module via the proxy `@latest` endpoint.
fn resolve_latest_version(module: &str) -> Result<String, DocsError> {
    let url = format!("{}/{}/@latest", GO_PROXY, escape_go_proxy(module));
    let body = crate::http::get(&url)?;
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| DocsError::ParseError(format!("invalid @latest JSON: {}", e)))?;
    v.get("Version")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| DocsError::NotFound(format!("no version found for module '{}'", module)))
}

/// Whether `module` is a Go standard-library package (no dot in the first segment).
fn is_stdlib_module(module: &str) -> bool {
    let first = module.split('/').next().unwrap_or(module);
    !first.contains('.')
}

/// Escape a module path or version for the Go module proxy.
///
/// The proxy requires uppercase ASCII letters to be encoded as `!` followed by the
/// lowercase letter (e.g. `github.com/Azure/foo` -> `github.com/!azure/foo`). This
/// avoids case-collision on case-insensitive filesystems.
fn escape_go_proxy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('!');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_go_proxy_lowercases_with_bang() {
        assert_eq!(
            escape_go_proxy("github.com/Azure/foo"),
            "github.com/!azure/foo"
        );
        assert_eq!(
            escape_go_proxy("github.com/Masterminds/semver"),
            "github.com/!masterminds/semver"
        );
        // No uppercase: unchanged.
        assert_eq!(
            escape_go_proxy("github.com/gin-gonic/gin"),
            "github.com/gin-gonic/gin"
        );
        // Versions are escaped too.
        assert_eq!(escape_go_proxy("v1.2.3"), "v1.2.3");
    }

    #[test]
    fn is_stdlib_module_detects_dotless_first_segment() {
        assert!(is_stdlib_module("fmt"));
        assert!(is_stdlib_module("encoding/json"));
        assert!(!is_stdlib_module("github.com/gin-gonic/gin"));
        assert!(!is_stdlib_module("golang.org/x/sync"));
    }

    #[test]
    fn version_from_cache_path_extracts_at_suffix() {
        let p = Path::new("/home/u/go/pkg/mod/github.com/gin-gonic/gin@v1.9.1/gin.go");
        assert_eq!(version_from_cache_path(p).as_deref(), Some("v1.9.1"));
        // stdlib-style path: no @.
        let p2 = Path::new("/usr/local/go/src/fmt/print.go");
        assert_eq!(version_from_cache_path(p2), None);
    }

    /// Offline test: build a tiny Go module source tree and extract a documented
    /// exported function through the local extractor's underlying tree walk.
    #[test]
    fn local_extractor_reads_doc_from_source_tree() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = "package greeter\n\
\n\
// Hello returns a greeting for the given name.\n\
func Hello(name string) string {\n\
\treturn \"hello \" + name\n\
}\n";
        std::fs::write(tmp.path().join("greeter.go"), src).unwrap();

        // The extractor delegates to extract_from_source_tree once the source dir is
        // resolved; exercise that final hop directly (resolution needs a real cache).
        let doc = doc_tree::extract_from_source_tree(
            tmp.path(),
            GO_GRAMMAR,
            "example.com/greeter",
            "greeter.Hello",
            "v1.0.0",
        )
        .expect("should extract Hello");
        assert_eq!(doc.name, "Hello");
        assert_eq!(doc.language, "go");
        assert_eq!(doc.package, "example.com/greeter");
        assert_eq!(doc.version, "v1.0.0");
        assert!(
            doc.signature.as_deref().unwrap().contains("Hello"),
            "signature: {:?}",
            doc.signature
        );
        assert!(
            doc.doc_body.contains("greeting"),
            "doc_body: {:?}",
            doc.doc_body
        );
    }

    /// Network test (proxy download) — ignored by default like the docs.rs tests.
    #[test]
    #[ignore = "network"]
    fn remote_fetch_gin_engine() {
        let doc = GoRemoteDocsFetcher
            .fetch_docs("github.com/gin-gonic/gin", "Engine", None)
            .expect("should fetch Engine from proxy");
        assert_eq!(doc.name, "Engine");
        assert_eq!(doc.language, "go");
        println!("{}", doc.doc_body);
    }
}
