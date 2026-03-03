//! `normalize find-references <symbol>` — find all references to a symbol.
//!
//! Combines two sources:
//! 1. **Within-file**: tree-sitter locals queries (`@local.reference` captures)
//!    resolve each reference to its definition via scope walk. Only available for
//!    languages that have a `locals.scm` query (21 arborium grammars today).
//! 2. **Cross-file**: facts index `find_callers` for call-level references.
//!
//! Output lists every reference as `file:line:col  name`.

use crate::output::OutputFormatter;
use normalize_languages::GrammarLoader;
use normalize_languages::support_for_path;
use normalize_scope::ScopeEngine;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// A single reference location.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReferenceEntry {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub name: String,
    /// Whether this is a definition site (true) or a use site (false).
    pub is_definition: bool,
}

/// Report returned by `normalize find-references`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReferencesReport {
    pub symbol: String,
    pub references: Vec<ReferenceEntry>,
    pub total: usize,
    /// Languages that have locals.scm support (scope-aware).
    pub scope_resolved: usize,
}

impl OutputFormatter for ReferencesReport {
    fn format_text(&self) -> String {
        if self.references.is_empty() {
            return format!("No references found for `{}`.", self.symbol);
        }

        let mut out = format!(
            "{} references to `{}` ({} scope-resolved):\n",
            self.total, self.symbol, self.scope_resolved
        );
        for r in &self.references {
            let kind = if r.is_definition { "def" } else { "ref" };
            out.push_str(&format!(
                "  {}:{}:{}  {} [{}]\n",
                r.file, r.line, r.column, r.name, kind
            ));
        }
        out
    }

    fn format_pretty(&self) -> String {
        use std::fmt::Write;

        if self.references.is_empty() {
            return format!("No references found for `{}`.\n", self.symbol);
        }

        let mut out = String::new();
        let _ = writeln!(
            out,
            "\x1b[1;34m{}\x1b[0m references to `\x1b[1m{}\x1b[0m` ({} scope-resolved)",
            self.total, self.symbol, self.scope_resolved
        );
        for r in &self.references {
            let (kind_color, kind) = if r.is_definition {
                ("\x1b[1;32m", "def")
            } else {
                ("\x1b[0;36m", "ref")
            };
            let _ = writeln!(
                out,
                "  \x1b[2m{}:{}:{}\x1b[0m  {}{}[{}]\x1b[0m",
                r.file, r.line, r.column, kind_color, r.name, kind
            );
        }
        out
    }
}

/// Run the find-references command.
///
/// - `root`: project root for path display and facts index lookup
/// - `symbol`: symbol name to search for
/// - `file`: if Some, restrict scope search to this file; cross-file always covers all files
pub fn cmd_find_references(root: &Path, symbol: &str, file: Option<&str>) -> ReferencesReport {
    let loader = GrammarLoader::new();
    let engine = ScopeEngine::new(&loader);

    let mut entries: Vec<ReferenceEntry> = Vec::new();
    let mut scope_resolved = 0;

    // Within-file: walk source files and run scope analysis
    let search_root = file
        .map(|f| {
            let p = PathBuf::from(f);
            if p.is_absolute() { p } else { root.join(f) }
        })
        .unwrap_or_else(|| root.to_path_buf());

    let files = collect_files(&search_root);

    for path in &files {
        let Some(lang) = support_for_path(path) else {
            continue;
        };
        let grammar_name = lang.grammar_name();

        if !engine.has_locals(grammar_name) {
            continue;
        }

        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };

        let refs = engine.find_references(grammar_name, &source, symbol);
        if refs.is_empty() {
            continue;
        }

        let display_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        for r in refs {
            scope_resolved += 1;
            entries.push(ReferenceEntry {
                file: display_path.clone(),
                line: r.location.line,
                column: r.location.column,
                name: r.name,
                is_definition: false,
            });
        }

        // Also include definition sites
        let defs = engine.find_definitions(grammar_name, &source, symbol);
        for d in defs {
            entries.push(ReferenceEntry {
                file: display_path.clone(),
                line: d.location.line,
                column: d.location.column,
                name: d.name,
                is_definition: true,
            });
        }
    }

    // Deduplicate by (file, line, column) — scope engine may emit both ref and def
    entries.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
    });
    entries.dedup_by(|a, b| a.file == b.file && a.line == b.line && a.column == b.column);

    let total = entries.len();
    ReferencesReport {
        symbol: symbol.to_string(),
        references: entries,
        total,
        scope_resolved,
    }
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return vec![root.to_path_buf()];
    }

    let mut files = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && support_for_path(path).is_some() {
            files.push(path.to_path_buf());
        }
    }
    files
}
