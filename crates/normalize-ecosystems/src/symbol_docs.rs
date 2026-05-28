//! Symbol documentation types for the `docs` command.
//!
//! `SymbolDoc` carries structured documentation for a single Rust (or future
//! language) symbol retrieved from upstream registries such as docs.rs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Source format of a symbol's documentation body.
///
/// Doc bodies are stored in their source-native format and rendered to display
/// form (Markdown) at the output layer, not at fetch time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DocFormat {
    /// CommonMark / Markdown (e.g. Rust `///` doc comments).
    Markdown,
    /// reStructuredText (e.g. Python docstrings).
    Rst,
    /// An HTML fragment (e.g. a rustdoc docblock from docs.rs).
    Html,
    /// Unstructured plain text.
    PlainText,
}

/// Documentation for a single named symbol from an upstream registry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SymbolDoc {
    /// Simple name of the symbol (e.g. "Serialize").
    pub name: String,
    /// Language ecosystem (e.g. "rust").
    pub language: String,
    /// Package / crate name (e.g. "serde").
    pub package: String,
    /// Resolved version string (e.g. "1.0.193").
    pub version: String,
    /// Full dotted symbol path (e.g. "serde::Serialize").
    pub symbol_path: String,
    /// Kind of item: "trait" | "struct" | "fn" | "enum" | "type" | "module" | ...
    pub kind: String,
    /// Formatted item declaration / signature, if available.
    pub signature: Option<String>,
    /// Primary documentation body, in its source-native format (see `doc_format`).
    pub doc_body: String,
    /// Source format of `doc_body`.
    pub doc_format: DocFormat,
    /// Runnable/shown examples extracted from the docs.
    pub examples: Vec<String>,
    /// Canonical source URL (e.g. `https://docs.rs/serde/1.0.193/serde/trait.Serialize.html`).
    pub source_url: String,
    /// When this record was fetched.
    #[schemars(with = "String")]
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

impl SymbolDoc {
    /// Stable knowledge-graph ID for this symbol doc.
    ///
    /// Shape: `docs-<language>-<package>-<version>-<escaped-symbol>`.
    /// Slashes, colons and other non-[a-z0-9] chars are replaced with `-`.
    pub fn kg_id(&self) -> String {
        let path_slug = self
            .symbol_path
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>();
        // Collapse consecutive dashes
        let path_slug = collapse_dashes(&path_slug);
        let version_slug = self.version.replace('.', "-");
        format!(
            "docs-{}-{}-{}-{}",
            self.language, self.package, version_slug, path_slug
        )
    }
}

fn collapse_dashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !last_dash {
                out.push('-');
            }
            last_dash = true;
        } else {
            out.push(c);
            last_dash = false;
        }
    }
    // Strip leading/trailing dashes
    out.trim_matches('-').to_string()
}
