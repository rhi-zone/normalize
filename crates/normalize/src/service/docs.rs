//! `normalize docs` — fetch upstream symbol documentation into LLM context.
//!
//! Looks up symbol docs from local Cargo source first, then falls back to
//! docs.rs when the package is not available locally. Results are cached in the
//! knowledge graph so repeat lookups are instant.
//!
//! ## Architecture
//!
//! Uses a two-trait coordinator pattern:
//! - [`CargoLocalDocsExtractor`] resolves packages via `cargo metadata` and
//!   parses doc comments from on-disk source. No network access.
//! - [`DocsRsFetcher`] fetches from docs.rs as the remote fallback.
//! - [`fetch_symbol_docs_with_fallback`] is the coordinator: local first, then remote.
//!
//! The `Ecosystem::fetch_symbol_docs` method on `Cargo` is retained for
//! backward compatibility but routes through the coordinator.
//!
//! ## Cache
//!
//! Results are stored as KG units under the ID scheme:
//!   `docs-cargo-<pkg>-<ver>-<slug>`
//! and are read back on subsequent invocations without touching the network.

use normalize_ecosystems::{
    CargoLocalDocsExtractor, DocsError, DocsRsFetcher, Ecosystem, SymbolDoc, ecosystems::Cargo,
    fetch_symbol_docs_with_fallback,
};
use normalize_knowledge_graph::{
    model::{Link, Unit},
    store::{ensure_kg_dir, kg_dir, read_unit, write_unit},
};
use std::path::PathBuf;

// ── Output type ───────────────────────────────────────────────────────────────

/// Output from `normalize docs`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct DocsReport {
    /// The Markdown block ready to paste into LLM context.
    pub markdown: String,
    /// Which package the symbol belongs to.
    pub package: String,
    /// Resolved version.
    pub version: String,
    /// Full symbol path queried.
    pub symbol_path: String,
    /// Item kind (trait / struct / fn / ...).
    pub kind: String,
    /// Whether the result was served from the local KG cache.
    pub from_cache: bool,
    /// Canonical source URL on docs.rs.
    pub source_url: String,
}

impl normalize_output::OutputFormatter for DocsReport {
    fn format_text(&self) -> String {
        self.markdown.clone()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse "pkg::Sym@version" or "pkg::Sym" into (symbol_path, explicit_version).
// normalize-syntax-allow: rust/tuple-return - private parsing helper, struct overhead unwarranted
pub(crate) fn parse_docs_query(input: &str) -> (String, Option<String>) {
    if let Some((path, ver)) = input.rsplit_once('@') {
        (path.to_string(), Some(ver.to_string()))
    } else {
        (input.to_string(), None)
    }
}

/// Extract the crate name from a symbol path like "serde::Serialize" → "serde".
pub(crate) fn crate_name_from_path(symbol_path: &str) -> &str {
    symbol_path.split("::").next().unwrap_or(symbol_path)
}

/// Find `Cargo.lock` walking up from `start` and return the pinned version of
/// `package` if present.
pub(crate) fn locked_version(package: &str, start: &std::path::Path) -> Option<String> {
    let cargo = Cargo;
    cargo.installed_version(package, start)
}

/// Store a `SymbolDoc` as a KG unit. Silently ignores errors (cache is best-effort).
pub(crate) fn cache_write(normalize_dir: &std::path::Path, doc: &SymbolDoc) {
    let Ok(kg) = ensure_kg_dir(normalize_dir) else {
        return;
    };
    let id = doc.kg_id();
    // Check ID validity (must be [a-z0-9-])
    if normalize_knowledge_graph::model::validate_id(&id).is_err() {
        return;
    }
    let metadata = serde_json::json!({
        "kind": "docs",
        "language": doc.language,
        "package": doc.package,
        "version": doc.version,
        "symbol_path": doc.symbol_path,
        "item_kind": doc.kind,
        "source_url": doc.source_url,
        "fetched_at": doc.fetched_at.to_rfc3339(),
    });
    let unit = Unit {
        id,
        metadata,
        links: vec![Link {
            kind: "source".to_string(),
            to: doc.source_url.clone(),
            metadata: serde_json::Value::Null,
        }],
        body: doc.to_markdown(),
    };
    let _ = write_unit(&kg, &unit);
}

pub(crate) fn cache_read_doc(normalize_dir: &std::path::Path, doc_id: &str) -> Option<DocsReport> {
    let kg = kg_dir(normalize_dir);
    let unit = read_unit(&kg, doc_id).ok()??;
    let meta = &unit.metadata;
    let package = meta.get("package")?.as_str()?.to_string();
    let version = meta.get("version")?.as_str()?.to_string();
    let symbol_path = meta.get("symbol_path")?.as_str()?.to_string();
    let kind = meta.get("item_kind")?.as_str()?.to_string();
    let source_url = meta.get("source_url")?.as_str()?.to_string();
    Some(DocsReport {
        markdown: unit.body,
        package,
        version,
        symbol_path,
        kind,
        from_cache: true,
        source_url,
    })
}

pub(crate) fn fetch_docs(
    symbol: &str,
    root_path: PathBuf,
    no_cache: bool,
) -> Result<DocsReport, String> {
    let (symbol_path, explicit_version) = parse_docs_query(symbol);
    let package = crate_name_from_path(&symbol_path).to_string();

    // Resolve version: explicit > lockfile > None (latest)
    let version: Option<String> = explicit_version.or_else(|| locked_version(&package, &root_path));

    // Cache lookup (only when version is known)
    let normalize_dir = root_path.join(".normalize");
    if !no_cache {
        if let Some(ver) = &version {
            let temp_doc = SymbolDoc {
                name: String::new(),
                language: "rust".to_string(),
                package: package.clone(),
                version: ver.clone(),
                symbol_path: symbol_path.clone(),
                kind: String::new(),
                signature: None,
                doc_text: String::new(),
                examples: vec![],
                source_url: String::new(),
                fetched_at: chrono::Utc::now(),
            };
            let id = temp_doc.kg_id();
            if let Some(report) = cache_read_doc(&normalize_dir, &id) {
                return Ok(report);
            }
        }
    }

    // Coordinator: local-first (cargo metadata + source parsing), then docs.rs fallback
    let local = CargoLocalDocsExtractor::new(&root_path);
    let remote = DocsRsFetcher;
    let doc = fetch_symbol_docs_with_fallback(
        &local,
        &remote,
        &package,
        &symbol_path,
        version.as_deref(),
    )
    .map_err(|e| format_docs_error_new(&e, &symbol_path))?;

    // Write to KG cache (best-effort)
    if !no_cache {
        cache_write(&normalize_dir, &doc);
    }

    let markdown = doc.to_markdown();
    Ok(DocsReport {
        markdown,
        package: doc.package,
        version: doc.version,
        symbol_path: doc.symbol_path,
        kind: doc.kind,
        from_cache: false,
        source_url: doc.source_url,
    })
}

pub(crate) fn format_docs_error_new(e: &DocsError, symbol_path: &str) -> String {
    match e {
        DocsError::NotFound(_) => format!(
            "Symbol '{}' not found. Check the crate name, symbol path, and version.",
            symbol_path
        ),
        DocsError::NetworkError(msg) => format!("docs.rs error: {}", msg),
        DocsError::ParseError(msg) => format!("Parse error fetching '{}': {}", symbol_path, msg),
        DocsError::ToolFailed(msg) => format!("Tool error for '{}': {}", symbol_path, msg),
    }
}
