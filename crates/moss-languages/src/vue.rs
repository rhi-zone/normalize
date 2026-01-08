//! Vue language support.

use crate::component::extract_embedded_content;
use crate::external_packages::ResolvedPackage;
use crate::{Export, Import, Language, Symbol, SymbolKind, Visibility, VisibilityMechanism};
use std::path::{Path, PathBuf};
use tree_sitter::Node;

/// Vue language support.
pub struct Vue;

impl Language for Vue {
    fn name(&self) -> &'static str {
        "Vue"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["vue"]
    }
    fn grammar_name(&self) -> &'static str {
        "vue"
    }

    fn has_symbols(&self) -> bool {
        true
    }

    fn container_kinds(&self) -> &'static [&'static str] {
        &["script_element", "template_element", "style_element"]
    }
    fn function_kinds(&self) -> &'static [&'static str] {
        &[] // JS functions are in embedded script, not Vue grammar
    }
    fn type_kinds(&self) -> &'static [&'static str] {
        &[]
    }
    fn import_kinds(&self) -> &'static [&'static str] {
        &[] // JS imports are in embedded script, not Vue grammar
    }

    fn public_symbol_kinds(&self) -> &'static [&'static str] {
        &[] // JS exports are in embedded script, not Vue grammar
    }

    fn visibility_mechanism(&self) -> VisibilityMechanism {
        VisibilityMechanism::ExplicitExport
    }

    fn scope_creating_kinds(&self) -> &'static [&'static str] {
        &["element"] // Vue template elements create scope
    }

    fn control_flow_kinds(&self) -> &'static [&'static str] {
        &["directive_attribute"] // v-if, v-for, v-show are directives
    }

    fn complexity_nodes(&self) -> &'static [&'static str] {
        &["directive_attribute", "interpolation"]
    }

    fn nesting_nodes(&self) -> &'static [&'static str] {
        &["element", "template_element", "script_element"]
    }

    fn signature_suffix(&self) -> &'static str {
        ""
    }

    fn extract_function(&self, node: &Node, content: &str, _in_container: bool) -> Option<Symbol> {
        let name = self.node_name(node, content)?;
        Some(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            signature: format!("function {}", name),
            docstring: None,
            attributes: Vec::new(),
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            visibility: Visibility::Public,
            children: Vec::new(),
            is_interface_impl: false,
            implements: Vec::new(),
        })
    }

    fn extract_container(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }

    fn extract_type(&self, _node: &Node, _content: &str) -> Option<Symbol> {
        None
    }
    fn extract_docstring(&self, _node: &Node, _content: &str) -> Option<String> {
        None
    }

    fn extract_attributes(&self, _node: &Node, _content: &str) -> Vec<String> {
        Vec::new()
    }
    fn extract_imports(&self, _node: &Node, _content: &str) -> Vec<Import> {
        Vec::new()
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        // Vue uses JS import syntax
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());
        if names_to_use.is_empty() {
            format!("import '{}';", import.module)
        } else {
            format!(
                "import {{ {} }} from '{}';",
                names_to_use.join(", "),
                import.module
            )
        }
    }
    fn extract_public_symbols(&self, _node: &Node, _content: &str) -> Vec<Export> {
        Vec::new()
    }

    fn is_public(&self, _node: &Node, _content: &str) -> bool {
        true
    }
    fn get_visibility(&self, _node: &Node, _content: &str) -> Visibility {
        Visibility::Public
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        {
            let name = symbol.name.as_str();
            match symbol.kind {
                crate::SymbolKind::Function | crate::SymbolKind::Method => {
                    name.starts_with("test_")
                        || name.starts_with("Test")
                        || name == "describe"
                        || name == "it"
                        || name == "test"
                }
                crate::SymbolKind::Module => {
                    name == "tests" || name == "test" || name == "__tests__"
                }
                _ => false,
            }
        }
    }

    fn embedded_content(&self, node: &Node, content: &str) -> Option<crate::EmbeddedBlock> {
        extract_embedded_content(node, content)
    }

    fn container_body<'a>(&self, node: &'a Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }
    fn body_has_docstring(&self, _body: &Node, _content: &str) -> bool {
        false
    }

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        let name_node = node.child_by_field_name("name")?;
        Some(&content[name_node.byte_range()])
    }

    fn file_path_to_module_name(&self, path: &Path) -> Option<String> {
        if path.extension()?.to_str()? != "vue" {
            return None;
        }
        Some(path.to_string_lossy().to_string())
    }
    fn module_name_to_paths(&self, module: &str) -> Vec<String> {
        vec![format!("{}.vue", module)]
    }

    fn lang_key(&self) -> &'static str {
        "vue"
    }
    fn resolve_local_import(&self, _: &str, _: &Path, _: &Path) -> Option<PathBuf> {
        None
    }
    fn resolve_external_import(&self, _: &str, _: &Path) -> Option<ResolvedPackage> {
        None
    }
    fn is_stdlib_import(&self, _: &str, _: &Path) -> bool {
        false
    }
    fn get_version(&self, _: &Path) -> Option<String> {
        None
    }
    fn find_package_cache(&self, _: &Path) -> Option<PathBuf> {
        None
    }
    fn indexable_extensions(&self) -> &'static [&'static str] {
        &["vue"]
    }
    fn find_stdlib(&self, _: &Path) -> Option<PathBuf> {
        None
    }
    fn package_module_name(&self, name: &str) -> String {
        name.strip_suffix(".vue").unwrap_or(name).to_string()
    }
    fn package_sources(&self, _: &Path) -> Vec<crate::PackageSource> {
        Vec::new()
    }
    fn discover_packages(&self, _: &crate::PackageSource) -> Vec<(String, PathBuf)> {
        Vec::new()
    }
    fn find_package_entry(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            Some(path.to_path_buf())
        } else {
            None
        }
    }

    fn should_skip_package_entry(&self, name: &str, is_dir: bool) -> bool {
        use crate::traits::{has_extension, skip_dotfiles};
        if skip_dotfiles(name) {
            return true;
        }
        if is_dir && name == "node_modules" {
            return true;
        }
        !is_dir && !has_extension(name, self.indexable_extensions())
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
            "directive_modifier", "directive_modifiers", "doctype",
        ];

        validate_unused_kinds_audit(&Vue, documented_unused)
            .expect("Vue unused node kinds audit failed");
    }
}
