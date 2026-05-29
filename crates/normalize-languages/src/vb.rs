//! Visual Basic language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::Path;
use tree_sitter::Node;

/// Visual Basic language support.
pub struct VB;

impl Language for VB {
    fn name(&self) -> &'static str {
        "Visual Basic"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["vb", "vbs"]
    }
    fn grammar_name(&self) -> &'static str {
        "vb"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn as_refactor_codegen(&self) -> Option<&dyn crate::RefactorCodeGen> {
        Some(self)
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "imports_statement" {
            return Vec::new();
        }

        let text = &content[node.byte_range()];
        vec![Import {
            module: text.trim().to_string(),
            names: Vec::new(),
            alias: None,
            is_wildcard: false,
            is_relative: false,
            line: node.start_position().row + 1,
        }]
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Visual Basic: Imports Namespace
        format!("Imports {}", import.module)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let text = &content[node.byte_range()];
        let lower = text.to_lowercase();
        if lower.contains("private") {
            Visibility::Private
        } else if lower.contains("protected") {
            Visibility::Protected
        } else {
            Visibility::Public
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
        &["**/*Test.vb", "**/*Tests.vb"]
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    fn analyze_container_body(
        &self,
        body_node: &Node,
        content: &str,
        inner_indent: &str,
    ) -> Option<ContainerBody> {
        crate::body::analyze_end_body(body_node, content, inner_indent)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: VBModuleResolver = VBModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for VB {}

impl crate::RefactorCodeGen for VB {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        match ty {
            Some(t) => format!("{} As {}", name, t),
            None => name.to_string(),
        }
    }

    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        format!("{}Dim {} = {}\n", indent, name, expr)
    }

    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        use crate::GenReturn;
        let param_str = spec
            .params
            .iter()
            .map(|p| match &p.inferred_type {
                Some(ty) => format!("{} As {}", p.name, ty),
                None => p.name.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let indent = &spec.indent;
        // `Sub` for no return, `Function ... As <T>` otherwise.
        let (keyword, ret_anno, end_kw) = match &spec.ret {
            GenReturn::Unit => ("Sub", String::new(), "Sub"),
            GenReturn::Single(v) => ("Function", format!(" As /* {} */", v), "Function"),
            GenReturn::Tuple(vs) => ("Function", format!(" As ({})", vs.join(", ")), "Function"),
            GenReturn::Result(ok, _) => ("Function", format!(" As /* {} */", ok), "Function"),
        };
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    Return {}", indent, v),
            GenReturn::Tuple(vs) => format!("\n{}    Return ({})", indent, vs.join(", ")),
            GenReturn::Result(ok, _) => format!("\n{}    Return {}", indent, ok),
        };

        let body = spec
            .body_lines
            .iter()
            .map(|l| format!("{}    {}", indent, l))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "\n{}Private {} {}({}){}\n{}{}\n{}End {}\n",
            indent, keyword, spec.name, param_str, ret_anno, body, return_stmt, indent, end_kw
        )
    }

    fn render_call_site(&self, spec: &crate::CallSiteSpec) -> String {
        use crate::GenReturn;
        let args = spec
            .params
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let indent = &spec.indent;
        let name = &spec.name;
        match &spec.ret {
            GenReturn::Unit => format!("{}{}({})\n", indent, name, args),
            GenReturn::Single(v) => format!("{}Dim {} = {}({})\n", indent, v, name, args),
            GenReturn::Tuple(vs) => {
                format!("{}Dim {} = {}({})\n", indent, vs.join(", "), name, args)
            }
            GenReturn::Result(ok, _) => format!("{}Dim {} = {}({})\n", indent, ok, name, args),
        }
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::VB;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    #[test]
    fn vb_fn_function() {
        let spec = ExtractedFnSpec {
            name: "Double".to_string(),
            params: vec![GenParam {
                name: "n".to_string(),
                inferred_type: Some("Integer".to_string()),
                mutable: false,
            }],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            is_generator: false,
            body_lines: vec!["Dim result = n * 2".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            VB.render_function(&spec),
            "\nPrivate Function Double(n As Integer) As /* result */\n    Dim result = n * 2\n    Return result\nEnd Function\n"
        );
    }

    #[test]
    fn vb_fn_sub() {
        let spec = ExtractedFnSpec {
            name: "Log".to_string(),
            params: vec![],
            ret: GenReturn::Unit,
            is_async: false,
            is_generator: false,
            body_lines: vec!["Console.WriteLine(x)".to_string()],
            indent: String::new(),
        };
        assert_eq!(
            VB.render_function(&spec),
            "\nPrivate Sub Log()\n    Console.WriteLine(x)\nEnd Sub\n"
        );
    }

    #[test]
    fn vb_call_site_and_binding() {
        let spec = CallSiteSpec {
            name: "Double".to_string(),
            params: vec![GenParam {
                name: "n".to_string(),
                inferred_type: None,
                mutable: false,
            }],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            indent: "    ".to_string(),
        };
        assert_eq!(VB.render_call_site(&spec), "    Dim result = Double(n)\n");
        assert_eq!(VB.render_binding("x", "F()", "  "), "  Dim x = F()\n");
        assert_eq!(VB.format_param("n", Some("Integer")), "n As Integer");
    }
}

// =============================================================================
// VB Module Resolver
// =============================================================================

/// Module resolver for Visual Basic .NET.
pub struct VBModuleResolver;

impl ModuleResolver for VBModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: vec![root.to_path_buf()],
        }
    }

    fn module_of_file(&self, root: &Path, file: &Path, _cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "vb" && ext != "vbs" {
            return Vec::new();
        }
        if let Ok(rel) = file.strip_prefix(root) {
            let rel_str = rel
                .to_str()
                .unwrap_or("")
                .trim_end_matches(".vbs")
                .trim_end_matches(".vb")
                .replace(['/', '\\'], ".");
            if !rel_str.is_empty() {
                return vec![ModuleId {
                    canonical_path: rel_str,
                }];
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "vb" && ext != "vbs" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        // Strip "Imports " prefix if present (VB stores full "Imports X.Y" as raw)
        let name = raw.strip_prefix("Imports ").unwrap_or(raw).trim();
        let exported_name = name.rsplit('.').next().unwrap_or(name).to_string();

        let parts: Vec<&str> = name.split('.').collect();
        for skip in 0..parts.len() {
            let path_part = parts[skip..].join("/");
            let candidate = cfg.workspace_root.join(format!("{}.vb", path_part));
            if candidate.exists() {
                return Resolution::Resolved(candidate, exported_name.clone());
            }
        }
        Resolution::NotFound
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
            // Block types
            "namespace_block",
            // Declaration types
            "field_declaration", "constructor_declaration", "event_declaration",
            "type_declaration", "const_declaration", "enum_member",
            // Statement types
            "statement", "assignment_statement", "compound_assignment_statement",
            "call_statement", "dim_statement", "redim_statement", "re_dim_clause",
            "exit_statement", "continue_statement", "return_statement", "goto_statement",
            "label_statement", "throw_statement", "empty_statement",
            // Control flow
            "try_statement", "catch_block", "finally_block",
            "case_block", "case_else_block", "else_clause", "elseif_clause",
            "with_statement", "with_initializer",
            "using_statement", "sync_lock_statement",
            // Expression types
            "expression", "binary_expression", "unary_expression", "ternary_expression",
            "parenthesized_expression", "lambda_expression", "new_expression",
            // Type-related
            "type", "generic_type", "array_type", "primitive_type",
            "type_parameters", "type_parameter", "type_constraint",
            "type_argument_list", "array_rank_specifier",
            // Clauses
            "as_clause", "inherits_clause", "implements_clause",
            // Modifiers
            "modifier", "modifiers",
            // Event handlers
            "add_handler_block", "remove_handler_block", "raise_event_block",
            // Other
            "identifier", "attribute_block", "option_statements",
            "relational_operator", "lambda_parameter",
            // control flow — not extracted as symbols
            "case_clause",
            "while_statement",
            "for_statement",
            "for_each_statement",
            "imports_statement",
            "do_statement",
            "if_statement",
            "select_case_statement",
        ];
        validate_unused_kinds_audit(&VB, documented_unused)
            .expect("Visual Basic unused node kinds audit failed");
    }
}
