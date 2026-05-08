//! Tree-sitter parser singleton and convenience functions.
//!
//! Provides a global `GrammarLoader` singleton so that grammars are loaded once
//! and shared across all call sites. This is the canonical way to parse source
//! code with tree-sitter in the normalize ecosystem.
//!
//! # Lifetime Safety
//!
//! The singleton is stored in a `'static OnceLock`, so the backing shared
//! libraries are never unloaded. This satisfies the lifetime requirement
//! documented in [`GrammarLoader`].
//!
//! # Missing-grammar reporting
//!
//! When a grammar fails to load (not installed, ABI mismatch, etc.), we emit
//! a single user-visible warning to stderr (deduplicated per process) and
//! record the failure in a process-wide tracker so callers like
//! `normalize structure rebuild` can summarise affected files. Use
//! [`try_get_grammar`] / [`parse_with_grammar`] / [`parser_for`] to get the
//! warning automatically; call [`report_missing_grammar`] directly if you
//! call [`GrammarLoader::get`] yourself.

use crate::{GrammarLoadError, GrammarLoader};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tree_sitter::Parser;

/// Global grammar loader singleton — avoids reloading grammars for each parse.
static GRAMMAR_LOADER: OnceLock<Arc<GrammarLoader>> = OnceLock::new();

/// Tracks grammars that have failed to load this process. Maps grammar name to
/// (warned_already, failure_count). The first failure prints a stderr warning;
/// subsequent failures only bump the count so callers can produce a summary
/// without spamming the user.
static MISSING_GRAMMARS: OnceLock<Mutex<HashMap<String, MissingGrammarRecord>>> = OnceLock::new();

#[derive(Debug, Clone)]
struct MissingGrammarRecord {
    /// Number of times this grammar was requested but failed to load.
    pub count: usize,
    /// Last error detail (used by the summary).
    pub detail: String,
}

/// Summary entry returned by [`take_missing_grammars`].
#[derive(Debug, Clone)]
pub struct MissingGrammar {
    /// Grammar name, e.g. `"go"`.
    pub name: String,
    /// Number of files / call sites that hit this missing grammar.
    pub count: usize,
    /// Human-readable error detail (e.g. "not found in search paths").
    pub detail: String,
}

fn missing_grammars() -> &'static Mutex<HashMap<String, MissingGrammarRecord>> {
    MISSING_GRAMMARS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record a grammar load failure and emit a one-shot stderr warning.
///
/// Subsequent calls with the same `name` only increment the failure count —
/// the user only sees the warning once per process per missing grammar. Use
/// [`take_missing_grammars`] at the end of a long-running command to print a
/// summary of affected files.
pub fn report_missing_grammar(name: &str, err: &GrammarLoadError) {
    let detail = format!("{err}");
    let mut map = missing_grammars().lock().unwrap_or_else(|e| e.into_inner());
    let entry = map
        .entry(name.to_string())
        .or_insert_with(|| MissingGrammarRecord {
            count: 0,
            detail: detail.clone(),
        });
    let first_time = entry.count == 0;
    entry.count += 1;
    entry.detail = detail;
    if first_time {
        eprintln!("warning: tree-sitter grammar '{name}' could not be loaded: {err}");
        eprintln!("    Run: normalize grammars install");
        eprintln!("    Or:  normalize grammars install --force  (if grammars are stale)");
    }
}

/// Drain and return the missing-grammar tracker.
///
/// Returns one entry per grammar that failed to load this process. The
/// internal counter is reset, so a subsequent rebuild starts fresh.
pub fn take_missing_grammars() -> Vec<MissingGrammar> {
    let mut map = missing_grammars().lock().unwrap_or_else(|e| e.into_inner());
    let drained: Vec<MissingGrammar> = map
        .drain()
        .map(|(name, rec)| MissingGrammar {
            name,
            count: rec.count,
            detail: rec.detail,
        })
        .collect();
    drained
}

/// Peek at the missing-grammar tracker without resetting it.
pub fn peek_missing_grammars() -> Vec<MissingGrammar> {
    let map = missing_grammars().lock().unwrap_or_else(|e| e.into_inner());
    map.iter()
        .map(|(name, rec)| MissingGrammar {
            name: name.clone(),
            count: rec.count,
            detail: rec.detail.clone(),
        })
        .collect()
}

/// Get the global grammar loader singleton.
pub fn grammar_loader() -> Arc<GrammarLoader> {
    GRAMMAR_LOADER
        .get_or_init(|| Arc::new(GrammarLoader::new()))
        .clone()
}

/// Try to load a grammar, surfacing missing-grammar failures as a one-shot
/// stderr warning. Returns `None` on any load failure (caller can short-circuit
/// like `?`).
pub fn try_get_grammar(grammar: &str) -> Option<tree_sitter::Language> {
    match grammar_loader().get(grammar) {
        Ok(lang) => Some(lang),
        Err(err) => {
            report_missing_grammar(grammar, &err);
            None
        }
    }
}

/// Create a parser for a specific grammar.
///
/// The grammar name should match tree-sitter grammar names
/// (e.g., "python", "rust", "typescript"). Emits a warning to stderr on the
/// first call where the grammar fails to load.
pub fn parser_for(grammar: &str) -> Option<Parser> {
    let language = try_get_grammar(grammar)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    Some(parser)
}

/// Parse source code with a specific grammar.
///
/// The grammar name should match tree-sitter grammar names
/// (e.g., "python", "rust", "typescript"). Emits a warning to stderr on the
/// first call where the grammar fails to load.
pub fn parse_with_grammar(grammar: &str, source: &str) -> Option<tree_sitter::Tree> {
    let mut parser = parser_for(grammar)?;
    parser.parse(source, None)
}

/// List grammars available in external search paths.
pub fn available_external_grammars() -> Vec<String> {
    grammar_loader().available_external()
}

/// List grammars available in external search paths, with their file paths.
pub fn available_external_grammars_with_paths() -> Vec<(String, std::path::PathBuf)> {
    grammar_loader().available_external_with_paths()
}
