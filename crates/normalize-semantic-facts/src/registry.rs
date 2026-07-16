//! The extractor registry: the single lookup point mapping tree-sitter
//! grammar names (e.g. `"typescript"`, `"tsx"`, `"sql"`) to the
//! [`SemanticFactExtractor`] that handles them.
//!
//! Callers never construct extractors directly — they go through
//! [`extractor_for_grammar`] (or, more commonly,
//! [`crate::extract_from_source`], which also handles parsing). This is what
//! lets new languages register semantic fact extraction without every
//! caller needing to know which extractor implements which grammar.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::extract::SemanticFactExtractor;
use crate::sql::SqlExtractor;
use crate::typescript::TypeScriptExtractor;

type Registry = HashMap<&'static str, Arc<dyn SemanticFactExtractor>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut map = Registry::new();
        register(&mut map, Arc::new(TypeScriptExtractor));
        register(&mut map, Arc::new(SqlExtractor));
        map
    })
}

/// Inserts `extractor` into `map` under every grammar name it reports via
/// [`SemanticFactExtractor::grammar_names`].
fn register(map: &mut Registry, extractor: Arc<dyn SemanticFactExtractor>) {
    for &name in extractor.grammar_names() {
        map.insert(name, extractor.clone());
    }
}

/// Looks up the extractor registered for tree-sitter grammar `name` (e.g.
/// `"typescript"`, `"tsx"`, `"sql"`). Returns `None` for any grammar with no
/// registered semantic fact extractor — most languages don't have one yet,
/// which is expected, not an error.
pub fn extractor_for_grammar(name: &str) -> Option<&'static dyn SemanticFactExtractor> {
    registry().get(name).map(|arc| arc.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_typescript_extractor_for_typescript_and_tsx() {
        let ts = extractor_for_grammar("typescript").expect("typescript should be registered");
        let tsx = extractor_for_grammar("tsx").expect("tsx should be registered");
        assert!(ts.grammar_names().contains(&"typescript"));
        assert!(ts.grammar_names().contains(&"tsx"));
        assert!(tsx.grammar_names().contains(&"typescript"));
        assert!(tsx.grammar_names().contains(&"tsx"));
    }

    #[test]
    fn returns_sql_extractor_for_sql() {
        let sql = extractor_for_grammar("sql").expect("sql should be registered");
        assert_eq!(sql.grammar_names(), &["sql"]);
    }

    #[test]
    fn returns_none_for_unknown_grammar() {
        assert!(extractor_for_grammar("python").is_none());
        assert!(extractor_for_grammar("rust").is_none());
        assert!(extractor_for_grammar("").is_none());
    }
}
