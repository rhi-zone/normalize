//! PHP language support.

use crate::traits::{ImportSpec, ModuleId, ModuleResolver, Resolution, ResolverConfig};
use crate::{ContainerBody, Import, Language, LanguageSymbols, Visibility};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// PHP language support.
pub struct Php;

impl Language for Php {
    fn name(&self) -> &'static str {
        "PHP"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["php", "phtml"]
    }
    fn grammar_name(&self) -> &'static str {
        "php"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        match node.kind() {
            "enum_declaration" => crate::SymbolKind::Enum,
            "interface_declaration" => crate::SymbolKind::Interface,
            "trait_declaration" => crate::SymbolKind::Trait,
            _ => tag_kind,
        }
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "attribute_list" {
                let mut ac = child.walk();
                for attr in child.children(&mut ac) {
                    if attr.kind() == "attribute_group" || attr.kind() == "attribute" {
                        attrs.push(content[attr.byte_range()].to_string());
                    }
                }
            }
        }
        // Also check preceding siblings (PHP attributes can precede the declaration)
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute_list" {
                let mut ac = sibling.walk();
                for attr in sibling.children(&mut ac) {
                    if attr.kind() == "attribute_group" || attr.kind() == "attribute" {
                        attrs.push(content[attr.byte_range()].to_string());
                    }
                }
                prev = sibling.prev_sibling();
                continue;
            }
            break;
        }
        attrs
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_phpdoc(node, content)
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                let text = &content[node.byte_range()];
                return text.lines().next().unwrap_or(text).trim().to_string();
            }
        };
        match node.kind() {
            "function_declaration" | "method_declaration" => {
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                let return_type = node
                    .child_by_field_name("return_type")
                    .map(|t| format!(": {}", content[t.byte_range()].trim()))
                    .unwrap_or_default();
                format!("function {}{}{}", name, params, return_type)
            }
            "interface_declaration" => format!("interface {}", name),
            "trait_declaration" => format!("trait {}", name),
            "enum_declaration" => format!("enum {}", name),
            "namespace_definition" => format!("namespace {}", name),
            _ => format!("class {}", name),
        }
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_clause" || child.kind() == "class_interface_clause" {
                let mut cl = child.walk();
                for t in child.children(&mut cl) {
                    if t.kind() == "name" || t.kind() == "qualified_name" {
                        implements.push(content[t.byte_range()].to_string());
                    }
                }
            }
        }
        crate::ImplementsInfo {
            is_interface: node.kind() == "interface_declaration",
            implements,
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "namespace_use_declaration" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;
        let mut imports = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "namespace_use_clause" {
                let text = content[child.byte_range()].to_string();
                imports.push(Import {
                    module: text,
                    names: Vec::new(),
                    alias: None,
                    is_wildcard: false,
                    is_relative: false,
                    line,
                });
            }
        }

        imports
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // PHP: use Namespace\Class;
        format!("use {};", import.module)
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
        &["**/*Test.php"]
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
        crate::body::analyze_brace_body(body_node, content, inner_indent)
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let mod_text = &content[child.byte_range()];
                if mod_text == "private" {
                    return Visibility::Private;
                }
                if mod_text == "protected" {
                    return Visibility::Protected;
                }
                if mod_text == "public" {
                    return Visibility::Public;
                }
            }
        }
        // PHP default visibility for methods/properties in classes is public
        Visibility::Public
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: PhpModuleResolver = PhpModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Php {}

// =============================================================================
// PHP Module Resolver
// =============================================================================

/// Module resolver for PHP (PSR-4 / composer.json conventions).
///
/// Reads `composer.json` `autoload.psr-4` to build namespace→directory mappings.
pub struct PhpModuleResolver;

impl ModuleResolver for PhpModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        let composer_json = root.join("composer.json");
        if let Ok(content) = std::fs::read_to_string(&composer_json)
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        {
            // Parse autoload.psr-4 and autoload-dev.psr-4
            for autoload_key in &["autoload", "autoload-dev"] {
                if let Some(psr4) = json
                    .get(autoload_key)
                    .and_then(|a| a.get("psr-4"))
                    .and_then(|p| p.as_object())
                {
                    for (namespace, dir) in psr4 {
                        let ns = namespace.trim_end_matches('\\').to_string();
                        if let Some(dir_str) = dir.as_str() {
                            let target = root.join(dir_str);
                            path_mappings.push((ns, target));
                        }
                    }
                }
            }
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots: vec![root.to_path_buf()],
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "php" && ext != "phtml" {
            return Vec::new();
        }
        for (ns, dir) in &cfg.path_mappings {
            if let Ok(rel) = file.strip_prefix(dir) {
                let rel_str = rel
                    .to_str()
                    .unwrap_or("")
                    .trim_end_matches(".phtml")
                    .trim_end_matches(".php")
                    .replace('/', "\\");
                let canonical = format!("{}\\{}", ns, rel_str);
                return vec![ModuleId {
                    canonical_path: canonical,
                }];
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "php" && ext != "phtml" {
            return Resolution::NotApplicable;
        }
        let raw = &spec.raw;
        let exported_name = raw.rsplit('\\').next().unwrap_or(raw).to_string();

        // Try PSR-4 mappings
        for (ns, dir) in &cfg.path_mappings {
            let ns_prefix = format!("{}\\", ns);
            if let Some(rest) = raw
                .strip_prefix(&ns_prefix)
                .or_else(|| if raw == ns { Some("") } else { None })
            {
                let file_path = rest.replace('\\', "/");
                let candidate = dir.join(format!("{}.php", file_path));
                if candidate.exists() {
                    return Resolution::Resolved(candidate, exported_name);
                }
            }
        }

        // Relative require/include
        if (spec.is_relative || raw.starts_with('.'))
            && let Some(parent) = from_file.parent()
        {
            let candidate = parent.join(raw);
            if candidate.exists() {
                return Resolution::Resolved(candidate, exported_name);
            }
        }

        Resolution::NotFound
    }
}

/// Extract a PHPDoc comment (`/** ... */`) preceding a PHP declaration.
fn extract_phpdoc(node: &Node, content: &str) -> Option<String> {
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        if sibling.kind() == "comment" {
            let text = &content[sibling.byte_range()];
            if text.starts_with("/**") {
                let lines: Vec<&str> = text
                    .strip_prefix("/**")
                    .unwrap_or(text)
                    .strip_suffix("*/")
                    .unwrap_or(text)
                    .lines()
                    .map(|l| l.trim().strip_prefix('*').unwrap_or(l).trim())
                    .filter(|l| !l.is_empty())
                    .collect();
                if !lines.is_empty() {
                    return Some(lines.join(" "));
                }
            }
            return None;
        }
        // Skip attributes between doc comment and declaration
        if sibling.kind() == "attribute_list" {
            prev = sibling.prev_sibling();
            continue;
        }
        return None;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "abstract_modifier", "anonymous_class", "anonymous_function",
            "anonymous_function_use_clause", "base_clause", "cast_expression", "cast_type",
            "class_constant_access_expression", "class_interface_clause", "colon_block",
            "compound_statement", "const_declaration", "declaration_list", "enum_case",
            "enum_declaration_list", "final_modifier", "formal_parameters", "heredoc_body",
            "named_type", "namespace_use_clause", "nowdoc_body",
            "optional_type", "primitive_type", "property_declaration", "qualified_name",
            "readonly_modifier", "reference_modifier", "static_modifier", "static_variable_declaration",
            "use_as_clause", "use_declaration", "use_instead_of_clause", "var_modifier",
            "visibility_modifier",
            // CLAUSE
            "declare_statement", "default_statement", "else_clause", "else_if_clause",
            "finally_clause", "match_block", "match_condition_list", "match_conditional_expression",
            "match_default_expression", "switch_block",
            // EXPRESSION
            "array_creation_expression", "assignment_expression", "augmented_assignment_expression",
            "binary_expression", "bottom_type", "clone_expression", "disjunctive_normal_form_type",
            "error_suppression_expression", "function_call_expression", "function_static_declaration",
            "include_expression", "include_once_expression", "intersection_type",
            "match_expression", "member_access_expression", "member_call_expression",
            "nullsafe_member_access_expression", "nullsafe_member_call_expression",
            "object_creation_expression", "parenthesized_expression", "reference_assignment_expression",
            "require_expression", "require_once_expression", "scoped_call_expression",
            "scoped_property_access_expression", "sequence_expression", "shell_command_expression",
            "subscript_expression", "type_list", "unary_op_expression", "union_type",
            "update_expression", "yield_expression",
            // STATEMENT
            "echo_statement", "empty_statement", "exit_statement", "expression_statement",
            "global_declaration", "goto_statement", "named_label_statement", "unset_statement",
            // control flow — not extracted as symbols
            "do_statement",
            "break_statement",
            "arrow_function",
            "if_statement",
            "for_statement",
            "return_statement",
            "foreach_statement",
            "case_statement",
            "namespace_use_declaration",
            "switch_statement",
            "throw_expression",
            "continue_statement",
            "catch_clause",
            "conditional_expression",
            "while_statement",
            "try_statement",
        ];

        validate_unused_kinds_audit(&Php, documented_unused)
            .expect("PHP unused node kinds audit failed");
    }
}
