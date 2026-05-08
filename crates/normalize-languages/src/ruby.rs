//! Ruby language support.

use std::path::Path;

use crate::{
    ContainerBody, Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver,
    Resolution, ResolverConfig, Visibility,
};
use tree_sitter::Node;

/// Ruby language support.
pub struct Ruby;

impl Language for Ruby {
    fn name(&self) -> &'static str {
        "Ruby"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["rb"]
    }
    fn grammar_name(&self) -> &'static str {
        "ruby"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        "; end"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        let mut doc_lines: Vec<String> = Vec::new();
        let mut prev = node.prev_sibling();

        while let Some(sibling) = prev {
            if sibling.kind() == "comment" {
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

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        let mut implements = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "superclass" {
                let mut sc = child.walk();
                for t in child.children(&mut sc) {
                    if t.kind() == "constant" || t.kind() == "scope_resolution" {
                        implements.push(content[t.byte_range()].to_string());
                    }
                }
            }
        }
        crate::ImplementsInfo {
            is_interface: false,
            implements,
        }
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        let name = match self.node_name(node, content) {
            Some(n) => n,
            None => {
                return content[node.byte_range()]
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
            }
        };
        match node.kind() {
            "method" | "singleton_method" => format!("def {}", name),
            "class" => format!("class {}", name),
            "module" => format!("module {}", name),
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn format_import(&self, import: &Import, _names: Option<&[&str]>) -> String {
        // Ruby: require 'x' or require_relative 'x'
        if import.is_relative {
            format!("require_relative '{}'", import.module)
        } else {
            format!("require '{}'", import.module)
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
        &[
            "**/spec/**/*.rb",
            "**/test/**/*.rb",
            "**/*_test.rb",
            "**/*_spec.rb",
        ]
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        // Ruby uses `private`, `protected`, `public` as method calls that change
        // visibility for all subsequent method definitions in the class body.
        // Walk backward through siblings to find the most recent visibility call.
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "call" || sibling.kind() == "identifier" {
                let text = &content[sibling.byte_range()];
                let method = text.split_whitespace().next().unwrap_or(text);
                match method {
                    "private" => return Visibility::Private,
                    "protected" => return Visibility::Protected,
                    "public" => return Visibility::Public,
                    _ => {}
                }
            }
            prev = sibling.prev_sibling();
        }
        // Ruby default is public
        Visibility::Public
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

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        extract_ruby_module_doc(src)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: RubyModuleResolver = RubyModuleResolver;
        Some(&RESOLVER)
    }
}

impl LanguageSymbols for Ruby {}

// =============================================================================
// Ruby Module Resolver
// =============================================================================

/// Module resolver for Ruby.
///
/// Handles `require_relative` imports (resolved against the caller's directory).
/// Bare `require` calls (Gem dependencies) return `NotFound`.
pub struct RubyModuleResolver;

impl ModuleResolver for RubyModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings: Vec::new(),
            search_roots: Vec::new(),
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "rb" {
            return Vec::new();
        }

        let rel = file.strip_prefix(&cfg.workspace_root).unwrap_or(file);

        let path_str = rel
            .components()
            .filter_map(|c| {
                if let std::path::Component::Normal(s) = c {
                    s.to_str()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("/");

        if path_str.is_empty() {
            return Vec::new();
        }

        // Strip .rb extension
        let canonical = path_str
            .strip_suffix(".rb")
            .unwrap_or(&path_str)
            .to_string();

        vec![ModuleId {
            canonical_path: canonical,
        }]
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        let ext = from_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "rb" {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // Only resolve require_relative (is_relative = true)
        if !spec.is_relative {
            return Resolution::NotFound;
        }

        let base_dir = from_file.parent().unwrap_or(&cfg.workspace_root);
        let candidate_base = base_dir.join(raw);

        // Try with .rb extension first, then as-is
        let with_rb = if candidate_base.extension().is_none() {
            let mut p = candidate_base.clone();
            p.set_extension("rb");
            p
        } else {
            candidate_base.clone()
        };

        if with_rb.exists() {
            return Resolution::Resolved(with_rb, String::new());
        }
        if candidate_base.exists() {
            return Resolution::Resolved(candidate_base, String::new());
        }

        Resolution::NotFound
    }
}

/// Extract the module-level doc comment from Ruby source.
///
/// Collects leading `#` comment lines, skipping `# frozen_string_literal` and
/// similar magic comment lines (which appear before actual doc comments).
fn extract_ruby_module_doc(src: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut past_magic = false;
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if lines.is_empty() {
                continue; // skip leading blank lines
            } else {
                break; // blank line ends the comment block
            }
        }
        if trimmed.starts_with('#') {
            let text = trimmed.strip_prefix('#').unwrap_or("").trim_start();
            // Skip magic comments: frozen_string_literal, encoding, etc.
            if !past_magic
                && (text.starts_with("frozen_string_literal")
                    || text.starts_with("encoding")
                    || text.starts_with("coding"))
            {
                continue;
            }
            past_magic = true;
            lines.push(text.to_string());
        } else {
            break; // non-comment, non-blank line ends the block
        }
    }
    if lines.is_empty() {
        return None;
    }
    // Strip trailing empty comment lines
    while lines.last().map(|l: &String| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
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
            // STRUCTURAL
            "begin_block", "block_argument", "block_body", "block_parameter", "block_parameters",
            "body_statement", "class_variable", "destructured_left_assignment",
            "destructured_parameter", "else", "elsif", "empty_statement", "end_block",
            "exception_variable", "exceptions", "expression_reference_pattern", "forward_argument",
            "forward_parameter", "heredoc_body", "lambda_parameters",
            "method_parameters", "operator", "operator_assignment", "parenthesized_statements", "superclass",
            // CLAUSE
            "case_match", "if_guard", "if_modifier", "in_clause", "match_pattern",
            "rescue_modifier", "unless_modifier", "until_modifier", "while_modifier",
            // EXPRESSION
            "yield",
            // control flow — not extracted as symbols
            "case",
            "while",
            "block",
            "retry",
            "do_block",
            "return",
            "for",
            "if",
            "lambda",
        ];

        validate_unused_kinds_audit(&Ruby, documented_unused)
            .expect("Ruby unused node kinds audit failed");
    }
}
