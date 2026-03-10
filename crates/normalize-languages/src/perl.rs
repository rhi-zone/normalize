//! Perl language support.

use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use tree_sitter::Node;

/// Perl language support.
pub struct Perl;

impl Language for Perl {
    fn name(&self) -> &'static str {
        "Perl"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["pl", "pm", "t"]
    }
    fn grammar_name(&self) -> &'static str {
        "perl"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // use Module::Name;
        // require Module::Name;
        let module = if let Some(rest) = text
            .strip_prefix("use ")
            .or_else(|| text.strip_prefix("require "))
        {
            rest.split([';', ' ']).next()
        } else {
            None
        };

        if let Some(module) = module {
            let module = module.trim().to_string();
            return vec![Import {
                module: module.clone(),
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: false,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Perl: use Module; or use Module qw(a b c);
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("use {};", import.module)
        } else {
            format!("use {} qw({});", import.module, names_to_use.join(" "))
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if self
            .node_name(node, content)
            .is_none_or(|n| !n.starts_with('_'))
        {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let name = symbol.name.as_str();
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => name.starts_with("test_"),
            crate::SymbolKind::Module => name == "tests" || name == "test",
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &["**/t/**/*.t", "**/*.t"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let mut doc_lines: Vec<String> = Vec::new();
        let mut prev = node.prev_sibling();

        while let Some(sibling) = prev {
            if sibling.kind() == "comment" || sibling.kind() == "comments" {
                let text = &content[sibling.byte_range()];
                if let Some(line) = text.strip_prefix('#') {
                    let line = line.strip_prefix(' ').unwrap_or(line);
                    doc_lines.push(line.to_string());
                } else {
                    break;
                }
            } else {
                break;
            }
            prev = sibling.prev_sibling();
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        let joined = doc_lines.join("\n").trim().to_string();
        if joined.is_empty() {
            None
        } else {
            Some(joined)
        }
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }
}

impl LanguageSymbols for Perl {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "ambiguous_function_call_expression", "amper_deref_expression",
            "anonymous_array_expression", "anonymous_hash_expression",
            "anonymous_method_expression", "anonymous_slice_expression",
            "anonymous_subroutine_expression", "array_deref_expression",
            "array_element_expression", "arraylen_deref_expression", "assignment_expression",
            "await_expression", "binary_expression", "block_statement", "class_phaser_statement",
            "class_statement", "coderef_call_expression",
            "defer_statement", "do_expression", "else", "elsif",
            "equality_expression", "eval_expression", "expression_statement",
            "fileglob_expression", "func0op_call_expression", "func1op_call_expression",
            "function", "function_call_expression", "glob_deref_expression",
            "glob_slot_expression", "goto_expression", "hash_deref_expression",
            "hash_element_expression", "identifier", "keyval_expression",
            "list_expression", "localization_expression",
            "loopex_expression", "lowprec_logical_expression", "map_grep_expression",
            "match_regexp", "match_regexp_modifiers", "method", "method_call_expression",
            "method_declaration_statement", "phaser_statement", "postfix_conditional_expression",
            "postfix_for_expression", "postfix_loop_expression", "postinc_expression",
            "preinc_expression", "prototype", "quoted_regexp_modifiers", "readline_expression",
            "refgen_expression", "relational_expression",
            "require_version_expression", "return_expression", "role_statement",
            "scalar_deref_expression", "slice_expression", "sort_expression", "statement_label",
            "stub_expression", "substitution_regexp_modifiers", "transliteration_expression",
            "transliteration_modifiers", "try_statement", "unary_expression", "undef_expression",
            "use_version_statement", "variable_declaration",
            // control flow — not extracted as symbols
            "for_statement",
            "conditional_statement",
            "loop_statement",
            "cstyle_for_statement",
            "require_expression",
            "block",
            "use_statement",
            "conditional_expression",
        ];
        validate_unused_kinds_audit(&Perl, documented_unused)
            .expect("Perl unused node kinds audit failed");
    }
}
