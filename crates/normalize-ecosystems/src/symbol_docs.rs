//! Symbol documentation types for the `docs` command.
//!
//! `SymbolDoc` carries structured documentation for a single Rust (or future
//! language) symbol retrieved from upstream registries such as docs.rs.

use serde::{Deserialize, Serialize};

/// Documentation for a single named symbol from an upstream registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Primary documentation body as Markdown.
    pub doc_text: String,
    /// Runnable/shown examples extracted from the docs.
    pub examples: Vec<String>,
    /// Canonical source URL (e.g. `https://docs.rs/serde/1.0.193/serde/trait.Serialize.html`).
    pub source_url: String,
    /// When this record was fetched.
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

impl SymbolDoc {
    /// Render the doc as a Markdown block suitable for pasting into LLM context.
    pub fn to_markdown(&self) -> String {
        // For crate-root docs (kind = "module", name = package), show just the package name.
        let heading = if self.name == self.package || self.symbol_path == self.package {
            self.package.clone()
        } else {
            format!("{}::{}", self.package, self.name)
        };
        let mut out = format!(
            "# {} (rust, {} {})\n\n",
            heading, self.package, self.version
        );

        out.push_str(&format!("{}\n\n", self.kind));

        if let Some(sig) = &self.signature {
            out.push_str("```rust\n");
            out.push_str(sig.trim());
            out.push_str("\n```\n\n");
        }

        if !self.doc_text.is_empty() {
            out.push_str(self.doc_text.trim());
            out.push_str("\n\n");
        }

        for (i, example) in self.examples.iter().enumerate() {
            if i == 0 {
                out.push_str("## Examples\n\n");
            }
            out.push_str("```rust\n");
            out.push_str(example.trim());
            out.push_str("\n```\n\n");
        }

        out.push_str(&format!("Source: <{}>\n", self.source_url));
        out
    }

    /// Stable knowledge-graph ID for this symbol doc.
    ///
    /// Shape: `docs-cargo-<package>-<version>-<escaped-symbol>`.
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
        format!("docs-cargo-{}-{}-{}", self.package, version_slug, path_slug)
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
