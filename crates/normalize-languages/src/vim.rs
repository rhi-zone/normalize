//! Vim script language support.

use crate::{Import, Language, Visibility};
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

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        if self
            .node_name(node, content)
            .is_none_or(|n| !n.starts_with("s:"))
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
            // Control flow — not definition constructs
            "elseif_statement", "for_loop", "if_statement", "source_statement",
            "try_statement", "runtime_statement", "while_loop",
                    // Previously in container/function/type_kinds, covered by tags.scm or needs review
            "function_definition",
        ];
        validate_unused_kinds_audit(&Vim, documented_unused)
            .expect("Vim unused node kinds audit failed");
    }
}
