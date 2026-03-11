//! Tree-sitter node-type inspector — list node kinds and field names for a grammar.

use normalize_languages::parsers::grammar_loader;
use normalize_output::OutputFormatter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Report returned by `normalize analyze node-types`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeTypesReport {
    /// Language name that was inspected.
    pub language: String,
    /// Count of named node kinds.
    pub named_kind_count: usize,
    /// Count of anonymous (literal/punctuation) node kinds.
    pub anonymous_kind_count: usize,
    /// Count of field names.
    pub field_count: usize,
    /// Named node kinds in sorted order (optionally filtered).
    pub named_kinds: Vec<String>,
    /// Anonymous node kinds in sorted order (optionally filtered).
    pub anonymous_kinds: Vec<String>,
    /// Field names in sorted order (optionally filtered).
    pub fields: Vec<String>,
}

impl OutputFormatter for NodeTypesReport {
    fn format_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Language: {} ({} named types, {} anonymous, {} fields)\n",
            self.language, self.named_kind_count, self.anonymous_kind_count, self.field_count
        ));
        if !self.named_kinds.is_empty() {
            out.push_str(&format!("\nNamed types ({}):\n", self.named_kinds.len()));
            for k in &self.named_kinds {
                out.push_str(&format!("  {k}\n"));
            }
        }
        if !self.fields.is_empty() {
            out.push_str(&format!("\nFields ({}):\n", self.fields.len()));
            for f in &self.fields {
                out.push_str(&format!("  {f}\n"));
            }
        }
        if !self.anonymous_kinds.is_empty() {
            out.push_str(&format!(
                "\nAnonymous types ({}):\n",
                self.anonymous_kinds.len()
            ));
            for k in &self.anonymous_kinds {
                out.push_str(&format!("  {k:?}\n"));
            }
        }
        out
    }

    fn format_pretty(&self) -> String {
        use nu_ansi_term::Color;
        let mut out = String::new();
        out.push_str(&format!(
            "Language: {} ({} named types, {} anonymous, {} fields)\n",
            Color::Cyan.bold().paint(&self.language),
            Color::Yellow.paint(self.named_kind_count.to_string()),
            self.anonymous_kind_count,
            Color::Green.paint(self.field_count.to_string())
        ));
        if !self.named_kinds.is_empty() {
            out.push_str(&format!(
                "\n{}:\n",
                Color::Cyan
                    .bold()
                    .paint(format!("Named types ({})", self.named_kinds.len()))
            ));
            for k in &self.named_kinds {
                out.push_str(&format!("  {}\n", Color::Cyan.paint(k)));
            }
        }
        if !self.fields.is_empty() {
            out.push_str(&format!(
                "\n{}:\n",
                Color::Green
                    .bold()
                    .paint(format!("Fields ({})", self.fields.len()))
            ));
            for f in &self.fields {
                out.push_str(&format!("  {}\n", Color::Green.paint(f)));
            }
        }
        if !self.anonymous_kinds.is_empty() {
            out.push_str(&format!(
                "\n{}:\n",
                Color::Yellow
                    .bold()
                    .paint(format!("Anonymous types ({})", self.anonymous_kinds.len()))
            ));
            for k in &self.anonymous_kinds {
                out.push_str(&format!("  {}\n", Color::Yellow.paint(format!("{k:?}"))));
            }
        }
        out
    }
}

/// List all node kinds and field names for the given language grammar.
///
/// - `search`: if supplied, filter all lists to entries containing this substring
///   (case-insensitive).
pub fn node_types_for_language(
    language_name: &str,
    search: Option<&str>,
) -> Result<NodeTypesReport, String> {
    let loader = grammar_loader();
    let ts_lang = loader
        .get(language_name)
        .ok_or_else(|| format!("grammar not loaded for language '{language_name}'"))?;

    let kind_count = ts_lang.node_kind_count();
    let field_count_raw = ts_lang.field_count();

    let search_lower = search.map(|s| s.to_lowercase());

    // Collect named and anonymous kinds.
    let mut named_kinds: Vec<String> = Vec::new();
    let mut anonymous_kinds: Vec<String> = Vec::new();

    for id in 0..kind_count as u16 {
        let kind = ts_lang.node_kind_for_id(id).unwrap_or("");
        if kind.is_empty() {
            continue;
        }
        let matches = search_lower
            .as_deref()
            .map(|s| kind.to_lowercase().contains(s))
            .unwrap_or(true);
        if !matches {
            continue;
        }
        if ts_lang.node_kind_is_named(id) {
            named_kinds.push(kind.to_string());
        } else {
            anonymous_kinds.push(kind.to_string());
        }
    }
    named_kinds.sort_unstable();
    named_kinds.dedup();
    anonymous_kinds.sort_unstable();
    anonymous_kinds.dedup();

    // Collect field names.
    let mut fields: Vec<String> = Vec::new();
    // Field IDs are 1-based; field_count() is the number of fields.
    for id in 1..=(field_count_raw as u16) {
        if let Some(name) = ts_lang.field_name_for_id(id) {
            let matches = search_lower
                .as_deref()
                .map(|s| name.to_lowercase().contains(s))
                .unwrap_or(true);
            if matches {
                fields.push(name.to_string());
            }
        }
    }
    fields.sort_unstable();

    // Use full unfiltered counts for the summary line.
    let named_kind_count = {
        let mut c = 0usize;
        for id in 0..kind_count as u16 {
            let kind = ts_lang.node_kind_for_id(id).unwrap_or("");
            if !kind.is_empty() && ts_lang.node_kind_is_named(id) {
                c += 1;
            }
        }
        c
    };
    let anonymous_kind_count = {
        let mut c = 0usize;
        for id in 0..kind_count as u16 {
            let kind = ts_lang.node_kind_for_id(id).unwrap_or("");
            if !kind.is_empty() && !ts_lang.node_kind_is_named(id) {
                c += 1;
            }
        }
        c
    };

    Ok(NodeTypesReport {
        language: language_name.to_string(),
        named_kind_count,
        anonymous_kind_count,
        field_count: field_count_raw,
        named_kinds,
        anonymous_kinds,
        fields,
    })
}
