//! Vim script language support.

use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use tree_sitter::Node;

/// Vim script language support.
pub struct Vim;

impl Language for Vim {
    fn name(&self) -> &'static str {
        "Vim"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["vim", "vimrc"]
    }
    fn grammar_name(&self) -> &'static str {
        "vim"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["function_definition", "augroup"]
    }

    fn function_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    fn import_kinds(&self) -> &'static [&'static str] {
        &["source_statement", "runtime_statement"]
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::NamingConvention // s: prefix for script-local
    }

    fn extract_public_symbols(&self, node: &Node, content: &str) -> Vec<Export> {
        let name = match self.node_name(node, content) {
            Some(n) => n.to_string(),
            None => return Vec::new(),
        };

        // s: prefix means script-local (private)
        if name.starts_with("s:") {
            return Vec::new();
        }

        vec![Export {
            name,
            kind: SymbolKind::Function,
            line: node.start_position().row + 1,
        }]
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["function_definition"]
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["if_statement", "for_loop", "while_loop", "try_statement"]
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["if_statement", "elseif_statement", "for_loop", "while_loop"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["function_definition", "if_statement", "for_loop"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        let text = &content[node.byte_range()];
        let first_line = text.lines().next().unwrap_or(text);

        let visibility = if name.starts_with("s:") {
            Visibility::Private
        } else {
            Visibility::Public
        };

        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: first_line.trim().to_string(),
            docstring: self.extract_docstring(node, content),
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, node: &Node, content: &str) -> Option<Symbol> {
        if node.kind() == "augroup" {
            let text = &content[node.byte_range()];
            let name = text
                .split_whitespace()
                .nth(1)
                .unwrap_or("unnamed")
                .to_string();
            return Some(Symbol {
                name: name.clone(),
                kind: SymbolKind::Module,
                signature: format!("augroup {}", name),
                docstring: None,
                attributes: Vec::new(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                visibility: Visibility::Public,
                children: Vec::new(),
                is_interface_impl: false,
                implements: Vec::new(),
            });
        }
        self.extract_function(node, content, false)
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        // Vim uses " for comments
        let mut prev = node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let text = &content[sibling.byte_range()];
            if sibling.kind() == "comment" && text.starts_with('"') {
                let line = text.strip_prefix('"').unwrap_or(text).trim();
                doc_lines.push(line.to_string());
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if doc_lines.is_empty() {
            return None;
        }

        doc_lines.reverse();
        Some(doc_lines.join(" "))
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        let text = &content[node.byte_range()];
        let line = node.start_position().row + 1;

        // source file.vim, runtime path/to/file.vim
        let module = text
            .strip_prefix("source ")
            .or_else(|| text.strip_prefix("runtime "))
            .map(|rest| rest.trim().to_string());

        if let Some(module) = module {
            return vec![Import {
                module,
                names: Vec::new(),
                alias: None,
                is_wildcard: false,
                is_relative: true,
                line,
            }];
        }

        Vec::new()
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Vim: source file.vim or runtime path/file.vim
        if import.is_relative {
            format!("source {}", import.module)
        } else {
            format!("runtime {}", import.module)
        }
    }

    fn is_public(&self, node: &Node, content: &str) -> bool {
        self.node_name(node, content)
            .is_none_or(|n| !n.starts_with("s:"))
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if self.is_public(node, content) {
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

    fn embedded_content(&self, _node: &Node, _content: &str) -> Option<crate::EmbeddedBlock> {
        None
    }

    fn container_body<'a>(&self, _node: &'a Node<'a>) -> Option<Node<'a>> {
        None
    }
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        node.child_by_field_name("name")
            .map(|n| &content[n.byte_range()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "aboveleft_statement", "autocmd_statement", "augroup_statement",
            "bang_filter_statement", "belowright_statement", "body", "botright_statement",
            "break_statement", "call_expression", "call_statement", "catch_statement",
            "cnext_statement", "colorscheme_statement", "comclear_statement",
            "command_statement", "const_statement", "continue_statement", "cprevious_statement",
            "delcommand_statement", "dictionnary_entry", "echo_statement", "echoerr_statement",
            "echohl_statement", "echomsg_statement", "echon_statement", "edit_statement",
            "else_statement", "enew_statement", "eval_statement", "ex_statement",
            "execute_statement", "field_expression", "file_format", "filetype",
            "filetype_statement", "filetypes", "finally_statement", "find_statement",
            "function_declaration", "global_statement", "highlight_statement", "identifier",
            "index_expression", "lambda_expression", "let_statement", "lua_statement",
            "map_statement", "marker_definition", "match_case", "method_expression",
            "normal_statement", "options_statement", "perl_statement", "python_statement",
            "range_statement", "register_statement", "return_statement", "ruby_statement",
            "scoped_identifier", "scriptencoding_statement", "set_statement",
            "setfiletype_statement", "setlocal_statement", "sign_statement", "silent_statement",
            "slice_expression", "startinsert_statement", "stopinsert_statement",
            "substitute_statement", "syntax_statement", "ternary_expression",
            "throw_statement", "topleft_statement", "unknown_builtin_statement",
            "unlet_statement", "vertical_statement", "view_statement", "visual_statement",
            "wincmd_statement",
        ];
        validate_unused_kinds_audit(&Vim, documented_unused)
            .expect("Vim unused node kinds audit failed");
    }
}
