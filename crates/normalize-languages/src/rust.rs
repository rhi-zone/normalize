//! Rust language support.

use std::path::{Path, PathBuf};

use crate::{
    ContainerBody, Import, ImportSpec, Language, LanguageSymbols, ModuleId, ModuleResolver,
    Resolution, ResolverConfig, Visibility,
};
use tree_sitter::Node;

/// Rust language support.
pub struct Rust;

impl Language for Rust {
    fn name(&self) -> &'static str {
        "Rust"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }
    fn grammar_name(&self) -> &'static str {
        "rust"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }

    fn as_refactor_codegen(&self) -> Option<&dyn crate::RefactorCodeGen> {
        Some(self)
    }

    fn signature_suffix(&self) -> &'static str {
        " {}"
    }

    fn extract_docstring(&self, node: &Node, content: &str) -> Option<String> {
        extract_docstring(node, content)
    }

    fn extract_attributes(&self, node: &Node, content: &str) -> Vec<String> {
        extract_attributes(node, content)
    }

    fn extract_implements(&self, node: &Node, content: &str) -> crate::ImplementsInfo {
        if node.kind() == "impl_item" {
            let type_node = match node.child_by_field_name("type") {
                Some(n) => n,
                None => return crate::ImplementsInfo::default(),
            };
            let _ = &content[type_node.byte_range()]; // used below
            let is_interface = node.child_by_field_name("trait").is_some();
            let implements = if let Some(trait_node) = node.child_by_field_name("trait") {
                vec![content[trait_node.byte_range()].to_string()]
            } else {
                Vec::new()
            };
            crate::ImplementsInfo {
                is_interface,
                implements,
            }
        } else {
            crate::ImplementsInfo::default()
        }
    }

    fn refine_kind(
        &self,
        node: &Node,
        _content: &str,
        tag_kind: crate::SymbolKind,
    ) -> crate::SymbolKind {
        match node.kind() {
            "struct_item" => crate::SymbolKind::Struct,
            "enum_item" => crate::SymbolKind::Enum,
            "type_item" => crate::SymbolKind::Type,
            "union_item" => crate::SymbolKind::Struct,
            "trait_item" => crate::SymbolKind::Trait,
            _ => tag_kind,
        }
    }

    fn build_signature(&self, node: &Node, content: &str) -> String {
        match node.kind() {
            "function_item" | "function_signature_item" => {
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
                let vis = self.extract_visibility_prefix(node, content);
                let params = node
                    .child_by_field_name("parameters")
                    .map(|p| content[p.byte_range()].to_string())
                    .unwrap_or_else(|| "()".to_string());
                let return_type = node
                    .child_by_field_name("return_type")
                    .map(|r| format!(" -> {}", &content[r.byte_range()]))
                    .unwrap_or_default();
                format!("{}fn {}{}{}", vis, name, params, return_type)
            }
            "impl_item" => {
                let type_node = node.child_by_field_name("type");
                let type_name = type_node
                    .map(|n| content[n.byte_range()].to_string())
                    .unwrap_or_default();
                if let Some(trait_node) = node.child_by_field_name("trait") {
                    let trait_name = &content[trait_node.byte_range()];
                    format!("impl {} for {}", trait_name, type_name)
                } else {
                    format!("impl {}", type_name)
                }
            }
            "trait_item" => {
                let name = self.node_name(node, content).unwrap_or("");
                let vis = self.extract_visibility_prefix(node, content);
                format!("{}trait {}", vis, name)
            }
            "mod_item" => {
                let name = self.node_name(node, content).unwrap_or("");
                let vis = self.extract_visibility_prefix(node, content);
                format!("{}mod {}", vis, name)
            }
            "struct_item" => {
                let name = self.node_name(node, content).unwrap_or("");
                let vis = self.extract_visibility_prefix(node, content);
                format!("{}struct {}", vis, name)
            }
            "enum_item" => {
                let name = self.node_name(node, content).unwrap_or("");
                let vis = self.extract_visibility_prefix(node, content);
                format!("{}enum {}", vis, name)
            }
            "type_item" => {
                let name = self.node_name(node, content).unwrap_or("");
                let vis = self.extract_visibility_prefix(node, content);
                format!("{}type {}", vis, name)
            }
            _ => {
                let text = &content[node.byte_range()];
                text.lines().next().unwrap_or(text).trim().to_string()
            }
        }
    }

    fn extract_imports(&self, node: &Node, content: &str) -> Vec<Import> {
        if node.kind() != "use_declaration" {
            return Vec::new();
        }

        let line = node.start_position().row + 1;
        let text = &content[node.byte_range()];
        let module = text.trim_start_matches("use ").trim_end_matches(';').trim();

        // Check for braced imports: use foo::{bar, baz}
        let mut names = Vec::new();
        let is_relative = module.starts_with("crate")
            || module.starts_with("self")
            || module.starts_with("super");

        if let Some(brace_start) = module.find('{') {
            let prefix = module[..brace_start].trim_end_matches("::");
            // Find matching closing brace using depth counter to handle nested groups
            // like `use std::{io::{Read, Write}, fs}`.
            let brace_end = {
                let mut depth = 0u32;
                let mut end = None;
                for (i, c) in module[brace_start..].char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = Some(brace_start + i);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                end
            };
            if let Some(brace_end) = brace_end {
                let items = &module[brace_start + 1..brace_end];
                for item in items.split(',') {
                    let trimmed = item.trim();
                    if !trimmed.is_empty() {
                        names.push(trimmed.to_string());
                    }
                }
            }
            vec![Import {
                module: prefix.to_string(),
                names,
                alias: None,
                is_wildcard: false,
                is_relative,
                line,
            }]
        } else {
            // Simple import: use foo::bar or use foo::bar as baz
            let (module_part, alias) = if let Some(as_pos) = module.find(" as ") {
                (&module[..as_pos], Some(module[as_pos + 4..].to_string()))
            } else {
                (module, None)
            };

            vec![Import {
                module: module_part.to_string(),
                names: Vec::new(),
                alias,
                is_wildcard: module_part.ends_with("::*"),
                is_relative,
                line,
            }]
        }
    }

    fn format_import(&self, import: &Import, names: Option<&[&str]>) -> String {
        let names_to_use: Vec<&str> = names
            .map(|n| n.to_vec())
            .unwrap_or_else(|| import.names.iter().map(|s| s.as_str()).collect());

        if import.is_wildcard {
            // Module already contains ::* from parsing
            format!("use {};", import.module)
        } else if names_to_use.is_empty() {
            format!("use {};", import.module)
        } else if names_to_use.len() == 1 {
            format!("use {}::{};", import.module, names_to_use[0])
        } else {
            format!("use {}::{{{}}};", import.module, names_to_use.join(", "))
        }
    }

    fn get_visibility(&self, node: &Node, content: &str) -> Visibility {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                let vis = &content[child.byte_range()];
                if vis == "pub" {
                    return Visibility::Public;
                } else if vis.starts_with("pub(crate)") {
                    return Visibility::Internal;
                } else if vis.starts_with("pub(super)") || vis.starts_with("pub(in") {
                    return Visibility::Protected;
                }
            }
        }
        Visibility::Private
    }

    fn is_test_symbol(&self, symbol: &crate::Symbol) -> bool {
        let in_attrs = symbol
            .attributes
            .iter()
            .any(|a| a.contains("#[test]") || a.contains("#[cfg(test)]"));
        let in_sig =
            symbol.signature.contains("#[test]") || symbol.signature.contains("#[cfg(test)]");
        if in_attrs || in_sig {
            return true;
        }
        match symbol.kind {
            crate::SymbolKind::Function | crate::SymbolKind::Method => {
                symbol.name.starts_with("test_")
            }
            crate::SymbolKind::Module => symbol.name == "tests",
            _ => false,
        }
    }

    fn test_file_globs(&self) -> &'static [&'static str] {
        &[
            "**/tests/**",
            "**/test_*.rs",
            "**/*_test.rs",
            "**/*_tests.rs",
        ]
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

    fn node_name<'a>(&self, node: &Node, content: &'a str) -> Option<&'a str> {
        // impl_item uses "type" field; trait_item and mod_item use "name"
        let name_node = node
            .child_by_field_name("name")
            .or_else(|| node.child_by_field_name("type"))?;
        Some(&content[name_node.byte_range()])
    }

    fn extract_module_doc(&self, src: &str) -> Option<String> {
        extract_rust_module_doc(src)
    }

    fn module_resolver(&self) -> Option<&dyn ModuleResolver> {
        static RESOLVER: RustModuleResolver = RustModuleResolver;
        Some(&RESOLVER)
    }

    fn post_process_symbols(
        &self,
        symbols: &mut Vec<crate::Symbol>,
        _resolver: Option<&dyn crate::InterfaceResolver>,
        _current_file: &str,
    ) {
        merge_rust_impl_blocks(symbols);
    }
}

impl LanguageSymbols for Rust {}

impl crate::RefactorCodeGen for Rust {
    fn format_param(&self, name: &str, ty: Option<&str>) -> String {
        match ty {
            Some(t) => format!("{}: {}", name, t),
            None => name.to_string(),
        }
    }

    fn render_binding(&self, name: &str, expr: &str, indent: &str) -> String {
        format!("{}let {} = {};\n", indent, name, expr)
    }

    fn render_function(&self, spec: &crate::ExtractedFnSpec) -> String {
        use crate::GenReturn;
        let async_kw = if spec.is_async { "async " } else { "" };
        let param_str = spec
            .params
            .iter()
            .map(|p| {
                let mut_kw = if p.mutable { "mut " } else { "" };
                match &p.inferred_type {
                    Some(ty) => format!("{}{}: {}", mut_kw, p.name, ty),
                    None => format!("{}{}: /* type */", mut_kw, p.name),
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let ret_str = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!(" -> /* {} */", v),
            GenReturn::Tuple(vs) => format!(" -> /* ({}) */", vs.join(", ")),
            GenReturn::Result(ok, err) => format!(" -> Result</* {} */, {}>", ok, err),
        };
        let indent = &spec.indent;
        let return_stmt = match &spec.ret {
            GenReturn::Unit => String::new(),
            GenReturn::Single(v) => format!("\n{}    {}", indent, v),
            GenReturn::Tuple(vs) => format!("\n{}    ({})", indent, vs.join(", ")),
            GenReturn::Result(ok, _) => {
                if ok == "()" {
                    format!("\n{}    Ok(())", indent)
                } else {
                    format!("\n{}    Ok({})", indent, ok)
                }
            }
        };

        let body = spec
            .body_lines
            .iter()
            .map(|l| format!("{}    {}", indent, l))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "\n{}{}fn {}({}){} {{\n{}{}\n{}}}\n",
            indent, async_kw, spec.name, param_str, ret_str, body, return_stmt, indent
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
        let is_async = spec.is_async;
        match &spec.ret {
            GenReturn::Unit => {
                if is_async {
                    format!("{}{}({}).await;\n", indent, name, args)
                } else {
                    format!("{}{}({});\n", indent, name, args)
                }
            }
            GenReturn::Single(v) => {
                if is_async {
                    format!("{}let {} = {}({}).await;\n", indent, v, name, args)
                } else {
                    format!("{}let {} = {}({});\n", indent, v, name, args)
                }
            }
            GenReturn::Tuple(vs) => {
                if is_async {
                    format!(
                        "{}let ({}) = {}({}).await;\n",
                        indent,
                        vs.join(", "),
                        name,
                        args
                    )
                } else {
                    format!("{}let ({}) = {}({});\n", indent, vs.join(", "), name, args)
                }
            }
            GenReturn::Result(ok, _) => {
                if is_async {
                    format!("{}let {} = {}({}).await?;\n", indent, ok, name, args)
                } else {
                    format!("{}let {} = {}({})?;\n", indent, ok, name, args)
                }
            }
        }
    }

    fn uses_result_for_exceptions(&self) -> bool {
        true
    }

    fn param_is_mutable(&self, content: &str, name: &str) -> bool {
        // Heuristic: look for `let mut <name>` in the source.
        // The index doesn't store mutability.
        let pattern = format!("let mut {}", name);
        content.contains(&pattern)
    }

    fn infer_param_type(&self, content: &str, name: &str) -> Option<String> {
        // Look for `let [mut] <name>: <type>` or `<name>: <type>` in function params.
        // Simple heuristic: find `<name>: ` and grab the type until `,`/`)`/`=`/newline.
        let pattern = format!("{}: ", name);
        if let Some(pos) = content.find(&pattern) {
            let after = &content[pos + pattern.len()..];
            let end = after.find([',', ')', '=', '\n']).unwrap_or(after.len());
            let ty = after[..end].trim().to_string();
            if !ty.is_empty() && !ty.contains(' ') {
                return Some(ty);
            }
        }
        None
    }
}

#[cfg(test)]
mod refactor_codegen_tests {
    use super::Rust;
    use crate::{CallSiteSpec, ExtractedFnSpec, GenParam, GenReturn, RefactorCodeGen};

    fn p(name: &str, ty: Option<&str>, mutable: bool) -> GenParam {
        GenParam {
            name: name.to_string(),
            inferred_type: ty.map(str::to_string),
            mutable,
        }
    }

    #[test]
    fn rust_fn_unit_return() {
        let spec = ExtractedFnSpec {
            name: "do_thing".to_string(),
            params: vec![p("x", Some("i32"), false)],
            ret: GenReturn::Unit,
            is_async: false,
            is_generator: false,
            body_lines: vec!["println!(\"{}\", x);".to_string()],
            indent: String::new(),
        };
        let out = Rust.render_function(&spec);
        assert!(out.contains("fn do_thing(x: i32)"));
        assert!(out.contains("println!"));
        assert!(!out.contains("->"));
    }

    #[test]
    fn rust_fn_single_return() {
        let spec = ExtractedFnSpec {
            name: "compute".to_string(),
            params: vec![],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            is_generator: false,
            body_lines: vec!["let result = 42;".to_string()],
            indent: String::new(),
        };
        let out = Rust.render_function(&spec);
        assert!(out.contains("fn compute()"));
        assert!(out.contains("-> /* result */"));
        assert!(out.contains("result"));
    }

    #[test]
    fn rust_fn_async() {
        let spec = ExtractedFnSpec {
            name: "wait_a_bit".to_string(),
            params: vec![],
            ret: GenReturn::Unit,
            is_async: true,
            is_generator: false,
            body_lines: vec!["tokio::time::sleep(Duration::from_secs(1)).await;".to_string()],
            indent: String::new(),
        };
        let out = Rust.render_function(&spec);
        assert!(out.contains("async fn wait_a_bit()"));
    }

    #[test]
    fn rust_call_site_with_return() {
        let spec = CallSiteSpec {
            name: "compute".to_string(),
            params: vec![p("x", Some("i32"), false)],
            ret: GenReturn::Single("result".to_string()),
            is_async: false,
            indent: "    ".to_string(),
        };
        assert_eq!(
            Rust.render_call_site(&spec),
            "    let result = compute(x);\n"
        );
    }

    #[test]
    fn rust_call_site_async() {
        let spec = CallSiteSpec {
            name: "fetch".to_string(),
            params: vec![],
            ret: GenReturn::Single("data".to_string()),
            is_async: true,
            indent: "    ".to_string(),
        };
        assert_eq!(
            Rust.render_call_site(&spec),
            "    let data = fetch().await;\n"
        );
    }
}

/// Merge Rust impl blocks with their corresponding struct/enum types.
///
/// Rust allows multiple `impl TypeName { ... }` blocks; tree-sitter tags each as a
/// separate top-level symbol. This pass folds all impl children and `implements` lists
/// into the matching struct/enum entry so the symbol tree reflects the logical type
/// rather than the syntactic impl layout.
fn merge_rust_impl_blocks(symbols: &mut Vec<crate::Symbol>) {
    use std::collections::HashMap;

    // Collect impl blocks: their children and implements lists
    let mut impl_methods: HashMap<String, Vec<crate::Symbol>> = HashMap::new();
    let mut impl_implements: HashMap<String, Vec<String>> = HashMap::new();

    // Remove impl blocks and collect their methods + implements
    symbols.retain(|sym| {
        if sym.signature.starts_with("impl ") {
            impl_methods
                .entry(sym.name.clone())
                .or_default()
                .extend(sym.children.clone());
            impl_implements
                .entry(sym.name.clone())
                .or_default()
                .extend(sym.implements.clone());
            return false;
        }
        true
    });

    // Add methods and implements to matching struct/enum
    for sym in symbols.iter_mut() {
        if matches!(
            sym.kind,
            crate::SymbolKind::Struct | crate::SymbolKind::Enum
        ) {
            if let Some(methods) = impl_methods.remove(&sym.name) {
                sym.children.extend(methods);
            }
            if let Some(impls) = impl_implements.remove(&sym.name) {
                sym.implements.extend(impls);
            }
        }
    }

    // Any remaining impl blocks without matching type: add back as module-like symbols
    for (name, methods) in impl_methods {
        let impls = impl_implements.remove(&name).unwrap_or_default();
        if !methods.is_empty() {
            symbols.push(crate::Symbol {
                name: name.clone(),
                kind: crate::SymbolKind::Module,
                signature: format!("impl {}", name),
                docstring: None,
                attributes: Vec::new(),
                start_line: methods.first().map(|m| m.start_line).unwrap_or(0),
                end_line: methods.last().map(|m| m.end_line).unwrap_or(0),
                visibility: crate::Visibility::Public,
                children: methods,
                is_interface_impl: !impls.is_empty(),
                implements: impls,
            });
        }
    }
}

/// Module resolver for Rust (Cargo workspace).
pub struct RustModuleResolver;

impl ModuleResolver for RustModuleResolver {
    fn workspace_config(&self, root: &Path) -> ResolverConfig {
        let cargo_toml = root.join("Cargo.toml");
        let mut path_mappings: Vec<(String, PathBuf)> = Vec::new();

        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            // Try workspace members
            if let Ok(val) = content.parse::<toml::Value>() {
                if let Some(members) = val
                    .get("workspace")
                    .and_then(|w| w.get("members"))
                    .and_then(|m| m.as_array())
                {
                    for member in members {
                        if let Some(member_str) = member.as_str() {
                            // Expand glob patterns (simple case: no actual glob, just list)
                            let member_path = root.join(member_str);
                            let member_cargo = member_path.join("Cargo.toml");
                            if let Ok(mc) = std::fs::read_to_string(&member_cargo)
                                && let Ok(mv) = mc.parse::<toml::Value>()
                                && let Some(name) = mv
                                    .get("package")
                                    .and_then(|p| p.get("name"))
                                    .and_then(|n| n.as_str())
                            {
                                path_mappings.push((name.to_string(), member_path));
                            }
                        }
                    }
                }

                // Single-crate: read package name from root Cargo.toml
                if path_mappings.is_empty()
                    && let Some(name) = val
                        .get("package")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                {
                    path_mappings.push((name.to_string(), root.to_path_buf()));
                }
            }
        }

        ResolverConfig {
            workspace_root: root.to_path_buf(),
            path_mappings,
            search_roots: Vec::new(),
        }
    }

    fn module_of_file(&self, _root: &Path, file: &Path, cfg: &ResolverConfig) -> Vec<ModuleId> {
        // Find the crate this file belongs to
        for (crate_name, crate_dir) in &cfg.path_mappings {
            let src_dir = crate_dir.join("src");
            if let Ok(rel) = file.strip_prefix(&src_dir) {
                let components: Vec<&str> = rel
                    .components()
                    .filter_map(|c| {
                        if let std::path::Component::Normal(s) = c {
                            s.to_str()
                        } else {
                            None
                        }
                    })
                    .collect();

                if components.is_empty() {
                    continue;
                }

                let last = *components.last().unwrap();
                let module_path =
                    if components.len() == 1 && (last == "lib.rs" || last == "main.rs") {
                        // Crate root
                        crate_name.clone()
                    } else {
                        // Build module path from components
                        let mut parts = vec![crate_name.as_str()];
                        for c in &components[..components.len() - 1] {
                            parts.push(c);
                        }
                        // Last component: strip .rs, handle mod.rs
                        let stem = if last == "mod.rs" {
                            // mod.rs is the module named by its parent directory
                            // (already included in parts above)
                            None
                        } else {
                            last.strip_suffix(".rs")
                        };
                        if let Some(s) = stem {
                            parts.push(s);
                        }
                        parts.join("::")
                    };

                return vec![ModuleId {
                    canonical_path: module_path,
                }];
            }
        }
        Vec::new()
    }

    fn resolve(&self, from_file: &Path, spec: &ImportSpec, cfg: &ResolverConfig) -> Resolution {
        // Only handle .rs files
        if from_file.extension().and_then(|e| e.to_str()) != Some("rs") {
            return Resolution::NotApplicable;
        }

        let raw = &spec.raw;

        // Handle super:: and self:: relative paths
        let resolved_raw = if raw.starts_with("super::") || raw.starts_with("self::") {
            // Resolve relative to from_file's module
            let crate_name = self.crate_name_for_file(from_file, cfg);
            if let Some(cn) = crate_name {
                let module_path = self.module_path_for_file(from_file, cfg);
                if let Some(suffix) = raw.strip_prefix("super::") {
                    // Pop one level from module path
                    let parent = module_path
                        .rsplit_once("::")
                        .map(|(p, _)| p.to_string())
                        .unwrap_or_else(|| cn.clone());
                    format!("{}::{}", parent, suffix)
                } else {
                    let suffix = raw.strip_prefix("self::").unwrap_or(raw);
                    format!("{}::{}", module_path, suffix)
                }
            } else {
                return Resolution::NotFound;
            }
        } else {
            raw.clone()
        };

        // Try to find the crate for this path
        let (crate_name, rest) = if let Some(pos) = resolved_raw.find("::") {
            let cn = &resolved_raw[..pos];
            let rest = &resolved_raw[pos + 2..];
            (cn.to_string(), rest.to_string())
        } else {
            (resolved_raw.clone(), String::new())
        };

        // Look up crate in workspace
        let crate_dir = cfg
            .path_mappings
            .iter()
            .find(|(name, _)| name == &crate_name)
            .map(|(_, dir)| dir.clone());

        let crate_dir = match crate_dir {
            Some(d) => d,
            None => {
                // Could be stdlib or external — not resolvable
                return Resolution::NotFound;
            }
        };

        // Convert module path to file path
        let exported_name = if let Some(pos) = rest.rfind("::") {
            rest[pos + 2..].to_string()
        } else {
            rest.clone()
        };

        let module_part = if let Some(pos) = rest.rfind("::") {
            rest[..pos].to_string()
        } else {
            String::new()
        };

        let candidate = self.module_to_file(&crate_dir, &module_part);
        if let Some(path) = candidate {
            Resolution::Resolved(path, exported_name)
        } else {
            Resolution::NotFound
        }
    }
}

impl RustModuleResolver {
    /// Get the crate name for a file.
    fn crate_name_for_file(&self, file: &Path, cfg: &ResolverConfig) -> Option<String> {
        for (crate_name, crate_dir) in &cfg.path_mappings {
            if file.starts_with(crate_dir) {
                return Some(crate_name.clone());
            }
        }
        None
    }

    /// Get the module path string for a file.
    fn module_path_for_file(&self, file: &Path, cfg: &ResolverConfig) -> String {
        for (crate_name, crate_dir) in &cfg.path_mappings {
            let src_dir = crate_dir.join("src");
            if let Ok(rel) = file.strip_prefix(&src_dir) {
                let components: Vec<&str> = rel
                    .components()
                    .filter_map(|c| {
                        if let std::path::Component::Normal(s) = c {
                            s.to_str()
                        } else {
                            None
                        }
                    })
                    .collect();
                if components.is_empty() {
                    return crate_name.clone();
                }
                let last = *components.last().unwrap();
                if components.len() == 1 && (last == "lib.rs" || last == "main.rs") {
                    return crate_name.clone();
                }
                let mut parts = vec![crate_name.as_str()];
                for c in &components[..components.len() - 1] {
                    parts.push(c);
                }
                if last != "mod.rs"
                    && let Some(s) = last.strip_suffix(".rs")
                {
                    parts.push(s);
                }
                return parts.join("::");
            }
        }
        String::new()
    }

    /// Convert a module path to a file path within a crate directory.
    ///
    /// e.g. "foo::bar" → tries `src/foo/bar.rs` and `src/foo/bar/mod.rs`
    fn module_to_file(&self, crate_dir: &Path, module_path: &str) -> Option<PathBuf> {
        let src_dir = crate_dir.join("src");

        if module_path.is_empty() {
            // Refers to the crate root
            let lib = src_dir.join("lib.rs");
            if lib.exists() {
                return Some(lib);
            }
            let main = src_dir.join("main.rs");
            if main.exists() {
                return Some(main);
            }
            return None;
        }

        let parts: Vec<&str> = module_path.split("::").collect();
        let mut path = src_dir.clone();
        for part in &parts {
            path = path.join(part);
        }

        // Try path.rs first
        let rs_path = path.with_extension("rs");
        if rs_path.exists() {
            return Some(rs_path);
        }

        // Try path/mod.rs
        let mod_path = path.join("mod.rs");
        if mod_path.exists() {
            return Some(mod_path);
        }

        None
    }
}

impl Rust {
    fn extract_visibility_prefix(&self, node: &Node, content: &str) -> String {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "visibility_modifier" {
                return format!("{} ", &content[child.byte_range()]);
            }
        }
        String::new()
    }
}

/// Extract a Rust doc comment from a node's `attributes` child.
///
/// Looks for `line_outer_doc_comment` nodes (`///`) and joins their text.
fn extract_docstring(node: &Node, content: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attributes" {
            let mut doc_lines = Vec::new();
            let mut attr_cursor = child.walk();
            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "line_outer_doc_comment" {
                    let text = &content[attr_child.byte_range()];
                    let doc = text.trim_start_matches("///").trim();
                    if !doc.is_empty() {
                        doc_lines.push(doc.to_string());
                    }
                }
            }
            if !doc_lines.is_empty() {
                return Some(doc_lines.join("\n"));
            }
        }
    }
    None
}

/// Extract Rust `#[...]` attribute items from a node.
///
/// Checks both the `attributes` child field and preceding sibling `attribute_item` nodes.
fn extract_attributes(node: &Node, content: &str) -> Vec<String> {
    let mut attrs = Vec::new();

    // Check for attributes child (e.g., #[test], #[cfg(test)])
    if let Some(attr_node) = node.child_by_field_name("attributes") {
        let mut cursor = attr_node.walk();
        for child in attr_node.children(&mut cursor) {
            if child.kind() == "attribute_item" {
                attrs.push(content[child.byte_range()].to_string());
            }
        }
    }

    // Also check preceding siblings for outer attributes
    let mut prev = node.prev_sibling();
    while let Some(sibling) = prev {
        if sibling.kind() == "attribute_item" {
            // Insert at beginning to maintain order
            attrs.insert(0, content[sibling.byte_range()].to_string());
            prev = sibling.prev_sibling();
        } else {
            break;
        }
    }

    attrs
}

/// Extract the module-level doc comment from Rust source.
///
/// Collects consecutive `//!` inner-doc comment lines from the top of the file,
/// stopping at the first line that is not a `//!` comment (ignoring blank lines).
fn extract_rust_module_doc(src: &str) -> Option<String> {
    let mut lines = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") {
            let text = trimmed.strip_prefix("//!").unwrap_or("").trim_start();
            lines.push(text.to_string());
        } else if trimmed.is_empty() && lines.is_empty() {
            // skip leading blank lines
        } else {
            break;
        }
    }
    if lines.is_empty() {
        return None;
    }
    // Strip trailing empty lines from the collected doc
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

    /// Documents node kinds that exist in the Rust grammar but aren't used in trait methods.
    /// Run `cross_check_node_kinds` in registry.rs to see all potentially useful kinds.
    #[test]
    fn unused_node_kinds_audit() {
        // Categories:
        // - STRUCTURAL: Internal/wrapper nodes
        // - CLAUSE: Sub-parts of larger constructs
        // - EXPRESSION: Expressions (we track statements/definitions)
        // - TYPE: Type-related nodes
        // - MODIFIER: Visibility/async/unsafe modifiers
        // - PATTERN: Pattern matching internals
        // - MACRO: Macro-related nodes
        // - MAYBE: Potentially useful

        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            // STRUCTURAL
            "block_comment",           // comments        // extern block contents
            "field_declaration",       // struct field
            "field_declaration_list",  // struct body
            "field_expression",        // foo.bar
            "field_identifier",        // field name
            "identifier",              // too common
            "lifetime",                // 'a
            "lifetime_parameter",      // <'a>
            "ordered_field_declaration_list", // tuple struct fields
            "scoped_identifier",       // path::to::thing
            "scoped_type_identifier",  // path::to::Type
            "shorthand_field_identifier", // struct init shorthand
            "type_identifier",         // type names
            "visibility_modifier",     // pub, pub(crate)

            // CLAUSE
            "else_clause",             // part of if
            "enum_variant",            // enum variant
            "enum_variant_list",       // enum body
            "match_block",             // match body
            "match_pattern",           // match arm pattern
            "trait_bounds",            // T: Foo + Bar
            "where_clause",            // where T: Foo

            // EXPRESSION
            "array_expression",        // [1, 2, 3]
            "assignment_expression",   // x = y
            "async_block",             // async { }
            "await_expression",        // foo.await         // foo()
            "generic_function",        // foo::<T>()
            "index_expression",        // arr[i]
            "parenthesized_expression",// (expr)
            "range_expression",        // 0..10
            "reference_expression",    // &x
            "struct_expression",       // Foo { x: 1 }
            "try_expression",          // foo?
            "tuple_expression",        // (a, b)
            "type_cast_expression",    // x as T
            "unary_expression",        // -x, !x
            "unit_expression",         // ()
            "yield_expression",        // yield x

            // TYPE
            "abstract_type",           // impl Trait
            "array_type",              // [T; N]
            "bounded_type",            // T: Foo
            "bracketed_type",          // <T>
            "dynamic_type",            // dyn Trait
            "function_type",           // fn(T) -> U
            "generic_type",            // Vec<T>
            "generic_type_with_turbofish", // Vec::<T>
            "higher_ranked_trait_bound", // for<'a>
            "never_type",              // !
            "pointer_type",            // *const T
            "primitive_type",          // i32, bool
            "qualified_type",          // <T as Trait>::Item
            "reference_type",          // &T
            "removed_trait_bound",     // ?Sized
            "tuple_type",              // (A, B)
            "type_arguments",          // <T, U>
            "type_binding",            // Item = T
            "type_parameter",          // T
            "type_parameters",         // <T, U>
            "unit_type",               // ()
            "unsafe_bound_type",       // unsafe trait bound

            // MODIFIER
            "block_outer_doc_comment", // //!
            "extern_modifier",         // extern "C"
            "function_modifiers",      // async, const, unsafe
            "mutable_specifier",       // mut

            // PATTERN
            "struct_pattern",          // Foo { x, y }
            "tuple_struct_pattern",    // Foo(x, y)

            // MACRO
            "fragment_specifier",      // $x:expr
            "macro_arguments_declaration", // macro args
            "macro_body_v2",           // macro body        // macro_rules!
            "macro_definition_v2",     // macro 2.0

            // OTHER
            "block_expression_with_attribute", // #[attr] { }
            "const_block",             // const { }
            "expression_statement",    // expr;
            "expression_with_attribute", // #[attr] expr
            "extern_crate_declaration",// extern crate
            "foreign_mod_item",        // extern block item
            "function_signature_item", // fn signature in trait
            "gen_block",               // gen { }
            "let_declaration",         // let x = y
            "try_block",               // try { }
            "unsafe_block",            // unsafe { }
            "use_as_clause",           // use foo as bar
            "empty_statement",         // ;
            // control flow — not extracted as symbols
            "closure_expression",
            "continue_expression",
            "match_expression",
            "use_declaration",
            "for_expression",
            "match_arm",
            "break_expression",
            "while_expression",
            "loop_expression",
            "return_expression",
            "if_expression",
            "block",
            "binary_expression",
        ];

        validate_unused_kinds_audit(&Rust, documented_unused)
            .expect("Rust unused node kinds audit failed");
    }
}
