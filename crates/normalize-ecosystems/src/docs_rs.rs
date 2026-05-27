//! Fetch symbol documentation from docs.rs for Rust crates.
//!
//! Strategy:
//! 1. Resolve version: use provided version, or look up latest from crates.io.
//! 2. Determine item kind and URL by trying docs.rs URL patterns in order
//!    (trait, struct, fn, enum, type, module/crate root).
//! 3. Fetch and parse the rustdoc HTML page.
//! 4. Extract: signature, doc text, code examples.
//!
//! Used as the [`RemoteDocsFetcher`] fallback for Cargo in the local-first
//! coordinator ([`crate::fetch_symbol_docs_with_fallback`]).

use crate::{DocsError, PackageError, RemoteDocsFetcher, symbol_docs::SymbolDoc};

// ── RemoteDocsFetcher impl ────────────────────────────────────────────────────

/// docs.rs fetcher — the remote fallback for Cargo symbol docs.
pub struct DocsRsFetcher;

impl RemoteDocsFetcher for DocsRsFetcher {
    fn fetch_docs(
        &self,
        package: &str,
        symbol_path: &str,
        version: Option<&str>,
    ) -> Result<SymbolDoc, DocsError> {
        fetch(package, symbol_path, version).map_err(DocsError::from)
    }
}

/// User-agent sent with all docs.rs / crates.io requests.
const USER_AGENT: &str = "normalize-docs-fetch/0 (https://github.com/rhi-zone/normalize)";

/// Ordered list of item kinds to try when the kind is not known in advance.
/// Determines the URL path fragment (`trait.Foo`, `struct.Foo`, etc.).
const KIND_PROBES: &[&str] = &[
    "trait", "struct", "enum", "fn", "type", "constant", "macro", "attr",
];

/// Resolve the latest version for a crate from crates.io.
pub fn resolve_latest_version(package: &str) -> Result<String, PackageError> {
    let url = format!("https://crates.io/api/v1/crates/{}", package);
    let body = crate::http::get_with_headers(&url, &[("User-Agent", USER_AGENT)])?;
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| PackageError::ParseError(format!("invalid JSON from crates.io: {}", e)))?;
    v.get("crate")
        .and_then(|c| c.get("newest_version"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| {
            PackageError::ParseError("missing newest_version in crates.io response".to_string())
        })
}

/// Fetch symbol docs from docs.rs.
///
/// `package` = crate name, `symbol_path` = full Rust path (e.g. "serde::Serialize"),
/// `version` = exact version or `None` for latest.
pub fn fetch(
    package: &str,
    symbol_path: &str,
    version: Option<&str>,
) -> Result<SymbolDoc, PackageError> {
    // 1. Resolve version
    let resolved_version = match version {
        Some(v) => v.to_string(),
        None => resolve_latest_version(package)?,
    };

    // 2. Parse symbol path into (module_prefix, item_name)
    //    e.g. "serde::Serialize" → module="" item="Serialize"
    //         "tokio::sync::Mutex" → module="sync" item="Mutex"
    //         "serde" → crate root
    let (module_path, item_name) = split_symbol_path(package, symbol_path);

    if item_name.is_none() {
        // Crate-level docs
        return fetch_crate_root(package, &resolved_version, symbol_path);
    }
    let item_name = item_name.unwrap();

    // 3. Try to find the item by probing candidate URLs
    let module_segment = module_path_to_url_segment(&module_path);

    // First try a crate index page to discover the actual kind
    let kind = probe_item_kind(package, &resolved_version, &module_segment, &item_name)?;

    // 4. Build the canonical URL and fetch
    let url = docs_rs_url(
        package,
        &resolved_version,
        &module_segment,
        &kind,
        &item_name,
    );
    let html = fetch_html(&url)?;

    // 5. Parse
    parse_symbol_page(
        package,
        &resolved_version,
        symbol_path,
        &item_name,
        &kind,
        &url,
        &html,
    )
}

/// Split "serde::Serialize" into (["serde"], "Serialize").
/// Returns (module_parts_excluding_crate, Some(item_name)) for items,
/// or ([], None) for crate-root queries.
// normalize-syntax-allow: rust/tuple-return - private parsing helper, struct overhead unwarranted
fn split_symbol_path(package: &str, symbol_path: &str) -> (Vec<String>, Option<String>) {
    // Check for crate-root query: either "serde" or "serde::" (with empty suffix)
    if symbol_path == package || symbol_path == &format!("{}::", package) {
        return (vec![], None);
    }

    // Strip leading crate name if present ("serde::Serialize" -> "Serialize")
    let stripped = if symbol_path.starts_with(&format!("{}::", package)) {
        &symbol_path[package.len() + 2..]
    } else {
        symbol_path
    };

    if stripped.is_empty() {
        return (vec![], None);
    }

    let parts: Vec<&str> = stripped.split("::").collect();
    if parts.len() == 1 {
        (vec![], Some(parts[0].to_string()))
    } else {
        let module = parts[..parts.len() - 1]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let item = parts[parts.len() - 1].to_string();
        (module, Some(item))
    }
}

fn module_path_to_url_segment(module: &Vec<String>) -> String {
    if module.is_empty() {
        String::new()
    } else {
        format!("{}/", module.join("/"))
    }
}

fn docs_rs_url(
    package: &str,
    version: &str,
    module_segment: &str,
    kind: &str,
    item: &str,
) -> String {
    format!(
        "https://docs.rs/{}/{}/{}/{}{}.\
         html",
        package,
        version,
        package.replace('-', "_"),
        module_segment,
        format!("{}.{}", kind, item),
    )
}

/// Probe the kind of a symbol by trying URL patterns in order.
fn probe_item_kind(
    package: &str,
    version: &str,
    module_segment: &str,
    item_name: &str,
) -> Result<String, PackageError> {
    let headers = &[("User-Agent", USER_AGENT)];

    for kind in KIND_PROBES {
        let url = docs_rs_url(package, version, module_segment, kind, item_name);
        // Use a HEAD-like check: fetch and see if we get a real page or 404
        match crate::http::get_with_headers(&url, headers) {
            Ok(body) if body.contains("class=\"docblock\"") || body.contains("item-decl") => {
                return Ok(kind.to_string());
            }
            Ok(_) => {
                // Got a response but it doesn't look like a symbol page;
                // might be a redirect to a different kind — continue probing
            }
            Err(PackageError::NotFound(_)) => continue,
            Err(e) => return Err(e),
        }
    }

    Err(PackageError::NotFound(format!(
        "symbol '{}' not found in {}/{}",
        item_name, package, version
    )))
}

/// Fetch the crate root docs page (index.html).
fn fetch_crate_root(
    package: &str,
    version: &str,
    symbol_path: &str,
) -> Result<SymbolDoc, PackageError> {
    let url = format!(
        "https://docs.rs/{}/{}/{}/index.html",
        package,
        version,
        package.replace('-', "_")
    );
    let html = fetch_html(&url)?;

    // Extract first docblock paragraph as doc_text
    let doc_text = extract_first_docblock_text(&html);

    Ok(SymbolDoc {
        name: package.to_string(),
        language: "rust".to_string(),
        package: package.to_string(),
        version: version.to_string(),
        symbol_path: symbol_path.to_string(),
        kind: "module".to_string(),
        signature: None,
        doc_text,
        examples: vec![],
        source_url: url,
        fetched_at: chrono::Utc::now(),
    })
}

fn fetch_html(url: &str) -> Result<String, PackageError> {
    crate::http::get_with_headers(url, &[("User-Agent", USER_AGENT)])
}

/// Parse a rustdoc HTML page for a symbol and extract structured data.
fn parse_symbol_page(
    package: &str,
    version: &str,
    symbol_path: &str,
    item_name: &str,
    kind: &str,
    url: &str,
    html: &str,
) -> Result<SymbolDoc, PackageError> {
    let signature = extract_signature(html);
    let doc_text = extract_first_docblock_text(html);
    let examples = extract_examples(html);

    Ok(SymbolDoc {
        name: item_name.to_string(),
        language: "rust".to_string(),
        package: package.to_string(),
        version: version.to_string(),
        symbol_path: symbol_path.to_string(),
        kind: kind.to_string(),
        signature,
        doc_text,
        examples,
        source_url: url.to_string(),
        fetched_at: chrono::Utc::now(),
    })
}

// ---------------------------------------------------------------------------
// HTML parsing helpers
// ---------------------------------------------------------------------------

/// Extract the item declaration from `<pre class="rust item-decl">`.
fn extract_signature(html: &str) -> Option<String> {
    let tag = "item-decl\">";
    let start_pos = html.find(tag)?;
    let after = &html[start_pos + tag.len()..];
    // Find inner <code>...</code>
    let code_start = after.find("<code>")?;
    let code_end = after.find("</code>")?;
    let raw = &after[code_start + 6..code_end];
    Some(strip_html_tags(raw).trim().to_string())
}

/// Extract the first doc-comment block from the main content area.
fn extract_first_docblock_text(html: &str) -> String {
    // Find the first <div class="docblock"> in the main content section
    // (skip sidebar docblocks if any appear before)
    let main_marker = "id=\"main-content\"";
    let search_from = html.find(main_marker).unwrap_or(0);
    let html_from_main = &html[search_from..];

    let tag = "class=\"docblock\"";
    let Some(start) = html_from_main.find(tag) else {
        return String::new();
    };
    let after = &html_from_main[start + tag.len()..];
    // Find the matching closing </div>
    let content = extract_until_closing_div(after);
    html_to_markdown(&content)
}

/// Extract code examples from `<div class="example-wrap">` blocks that contain
/// `<pre class="rust">` (or similar) within docblock content.
fn extract_examples(html: &str) -> Vec<String> {
    let mut examples = Vec::new();
    let main_marker = "id=\"main-content\"";
    let search_from = html.find(main_marker).unwrap_or(0);
    let html_from_main = &html[search_from..];

    // Find the first docblock to scope to (avoid impl-level examples)
    let docblock_tag = "class=\"docblock\"";
    let Some(docblock_start) = html_from_main.find(docblock_tag) else {
        return examples;
    };
    let docblock_html = &html_from_main[docblock_start..];
    // Take only up to the next h2/section (roughly the first docblock)
    let end = docblock_html
        .find("</section>")
        .unwrap_or(docblock_html.len().min(50_000));
    let docblock_html = &docblock_html[..end];

    let mut pos = 0;
    while let Some(wrap_start) = docblock_html[pos..].find("example-wrap") {
        let abs = pos + wrap_start;
        let inner = &docblock_html[abs..];
        // Find the <pre> inside
        if let Some(pre_start) = inner.find("<pre class=\"rust") {
            let pre_content = &inner[pre_start..];
            if let Some(code_start) = pre_content.find("<code>") {
                if let Some(code_end) = pre_content.find("</code>") {
                    let raw = &pre_content[code_start + 6..code_end];
                    let clean = strip_html_tags(raw).trim().to_string();
                    if !clean.is_empty() {
                        examples.push(clean);
                    }
                }
            }
        }
        pos = abs + 12; // past "example-wrap"
        if pos >= docblock_html.len() {
            break;
        }
    }

    examples
}

/// Extract content until the matching `</div>` for the current div.
/// Handles nesting by counting open/close div tags.
fn extract_until_closing_div(html: &str) -> String {
    let mut depth = 1i32;
    let mut pos = 0;
    let bytes = html.as_bytes();
    while pos < bytes.len() && depth > 0 {
        if bytes[pos] == b'<' {
            let slice = &html[pos..];
            if slice.starts_with("</div") {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                pos += 5;
            } else if slice.starts_with("<div") {
                depth += 1;
                pos += 4;
            } else {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }
    html[..pos].to_string()
}

/// Strip HTML tags, decode common entities, normalise whitespace.
fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_html_entities(&out)
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#x2212;", "−")
        .replace("&#x27;", "'")
}

/// Convert a fragment of rustdoc HTML to approximate Markdown.
/// This is best-effort: handles paragraphs, code spans, code blocks, links.
fn html_to_markdown(html: &str) -> String {
    // Simplistic approach: strip most tags, but preserve structure
    let mut out = String::new();

    // Replace <p> with newlines
    let html = html.replace("<p>", "").replace("</p>", "\n\n");
    // Code spans
    let html = html.replace("<code>", "`").replace("</code>", "`");
    // Strong/em
    let html = html
        .replace("<strong>", "**")
        .replace("</strong>", "**")
        .replace("<em>", "*")
        .replace("</em>", "*");
    // Headings
    let html = html
        .replace("<h1>", "# ")
        .replace("</h1>", "\n")
        .replace("<h2>", "## ")
        .replace("</h2>", "\n")
        .replace("<h3>", "### ")
        .replace("</h3>", "\n");
    // Links: <a ...>text</a> → text
    // Simple strip of all remaining tags
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }

    let out = decode_html_entities(&out);
    // Normalize multiple blank lines
    let out = normalize_blank_lines(&out);
    out.trim().to_string()
}

fn normalize_blank_lines(s: &str) -> String {
    let mut out = String::new();
    let mut blank_count = 0u32;
    for line in s.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                out.push('\n');
            }
        } else {
            blank_count = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_symbol_path_crate_root() {
        let (module, item) = split_symbol_path("serde", "serde");
        assert!(module.is_empty());
        assert!(item.is_none());
    }

    #[test]
    fn test_split_symbol_path_top_level() {
        let (module, item) = split_symbol_path("serde", "serde::Serialize");
        assert!(module.is_empty());
        assert_eq!(item.as_deref(), Some("Serialize"));
    }

    #[test]
    fn test_split_symbol_path_nested() {
        let (module, item) = split_symbol_path("tokio", "tokio::sync::Mutex");
        assert_eq!(module, vec!["sync"]);
        assert_eq!(item.as_deref(), Some("Mutex"));
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(
            strip_html_tags("<p>Hello <code>world</code></p>"),
            "Hello world"
        );
    }

    #[test]
    fn test_decode_entities() {
        assert_eq!(decode_html_entities("a &lt; b &amp; c"), "a < b & c");
    }

    #[test]
    fn test_docs_rs_url() {
        assert_eq!(
            docs_rs_url("serde", "1.0.193", "", "trait", "Serialize"),
            "https://docs.rs/serde/1.0.193/serde/trait.Serialize.html"
        );
        assert_eq!(
            docs_rs_url("tokio", "1.35.0", "sync/", "struct", "Mutex"),
            "https://docs.rs/tokio/1.35.0/tokio/sync/struct.Mutex.html"
        );
    }

    #[test]
    #[ignore = "network"]
    fn test_fetch_serde_serialize() {
        let doc = fetch("serde", "serde::Serialize", None).unwrap();
        assert_eq!(doc.name, "Serialize");
        assert_eq!(doc.package, "serde");
        assert_eq!(doc.kind, "trait");
        assert!(!doc.doc_text.is_empty(), "doc_text should not be empty");
        println!("{}", doc.to_markdown());
    }

    #[test]
    #[ignore = "network"]
    fn test_fetch_tokio_mutex() {
        let doc = fetch("tokio", "tokio::sync::Mutex", None).unwrap();
        assert_eq!(doc.name, "Mutex");
        assert_eq!(doc.kind, "struct");
        println!("{}", doc.to_markdown());
    }
}
