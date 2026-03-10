//! Uiua array programming language support.

use crate::{Language, LanguageSymbols};

/// Uiua language support.
pub struct Uiua;

impl Language for Uiua {
    fn name(&self) -> &'static str {
        "Uiua"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ua"]
    }
    fn grammar_name(&self) -> &'static str {
        "uiua"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }
}

impl LanguageSymbols for Uiua {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // Functions and modifiers
            "function", "inlineFunction", "switchFunctions",
            "modifier1", "modifier2",
            // Other
            "module", "identifier", "identifierDeprecated", "formatter",
        ];
        validate_unused_kinds_audit(&Uiua, documented_unused)
            .expect("Uiua unused node kinds audit failed");
    }
}
