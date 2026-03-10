//! AWK language support.

use crate::{Language, LanguageSymbols};

/// AWK language support.
pub struct Awk;

impl Language for Awk {
    fn name(&self) -> &'static str {
        "AWK"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["awk", "gawk"]
    }
    fn grammar_name(&self) -> &'static str {
        "awk"
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

impl LanguageSymbols for Awk {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "break_statement", "continue_statement", "delete_statement", "do_while_statement",
            "else_clause", "exit_statement", "identifier", "next_statement", "nextfile_statement",
            "ns_qualified_name", "piped_io_statement", "print_statement", "printf_statement",
            "redirected_io_statement", "return_statement", "switch_body", "switch_case",
            "switch_statement",
            // control flow — not extracted as symbols
            "if_statement",
            "for_in_statement",
            "for_statement",
            "while_statement",
            "block",
        ];
        validate_unused_kinds_audit(&Awk, documented_unused)
            .expect("AWK unused node kinds audit failed");
    }
}
