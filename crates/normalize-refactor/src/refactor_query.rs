//! Shared loader for the `*.refactor.scm` structural-classification queries.
//!
//! The refactoring recipes classify tree-sitter nodes (which node is a function
//! definition, a parameter list, a call, a variable declaration, a scope, a
//! statement, …) by *capture membership* in the per-language `refactor` query —
//! never by hardcoded `match grammar` node-kind dispatch. This module runs the
//! query once over a parsed tree and collects the matched node IDs into one
//! `HashSet<usize>` per capture name, mirroring how `actions::decoration_extended_start`
//! consumes the `decorations` query.

use std::collections::HashSet;

use normalize_languages::parsers::grammar_loader;
use normalize_languages::satisfies_predicates;
use tree_sitter::{Node, StreamingIterator as _};

/// Node-ID sets for every `@refactor.*` capture, keyed by capture name (without
/// the leading `refactor.`). A node is classified as e.g. a "function_def" iff
/// its `id()` is present in `self.get("function_def")`.
pub struct RefactorCaptures {
    sets: std::collections::HashMap<String, HashSet<usize>>,
}

impl RefactorCaptures {
    /// Run the `refactor` query for `grammar` over `tree`, collecting capture
    /// membership. Returns `None` when no `*.refactor.scm` exists for the grammar
    /// (i.e. the language is not supported by the refactoring engine) or the
    /// grammar/query fails to load/compile.
    pub fn load(grammar: &str, root: Node<'_>, content: &str) -> Option<Self> {
        let loader = grammar_loader();
        let query_src = loader.get_refactor(grammar)?;
        let compiled = loader.get_compiled_query(grammar, "refactor", &query_src)?;

        let mut sets: std::collections::HashMap<String, HashSet<usize>> =
            std::collections::HashMap::new();
        let capture_names = compiled.capture_names();

        let mut qcursor = tree_sitter::QueryCursor::new();
        let source_bytes = content.as_bytes();
        let mut matches = qcursor.matches(&compiled, root, source_bytes);
        while let Some(m) = matches.next() {
            if !satisfies_predicates(&compiled, m, source_bytes) {
                continue;
            }
            for capture in m.captures {
                let full = capture_names[capture.index as usize];
                let short = full.strip_prefix("refactor.").unwrap_or(full);
                sets.entry(short.to_string())
                    .or_default()
                    .insert(capture.node.id());
            }
        }

        Some(Self { sets })
    }

    /// True if `node` was captured under `@refactor.<capture>`.
    pub fn is(&self, capture: &str, node: &Node<'_>) -> bool {
        self.sets
            .get(capture)
            .is_some_and(|s| s.contains(&node.id()))
    }
}
