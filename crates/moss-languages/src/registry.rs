//! Language support registry with extension-based lookup.

use crate::Language;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

/// Global language registry.
static LANGUAGES: RwLock<Vec<&'static dyn Language>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Cached extension → language lookup table.
static EXTENSION_MAP: OnceLock<HashMap<&'static str, &'static dyn Language>> = OnceLock::new();

/// Cached grammar_name → language lookup table.
static GRAMMAR_MAP: OnceLock<HashMap<&'static str, &'static dyn Language>> = OnceLock::new();

/// Register a language in the global registry.
/// Called internally by language modules.
pub fn register(lang: &'static dyn Language) {
    LANGUAGES.write().unwrap().push(lang);
}

/// Initialize built-in languages (called once).
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        register(&crate::python::Python);

        register(&crate::rust::Rust);

        register(&crate::javascript::JavaScript);

        register(&crate::typescript::TypeScript);

        register(&crate::typescript::Tsx);

        register(&crate::go::Go);

        register(&crate::java::Java);

        register(&crate::kotlin::Kotlin);

        register(&crate::csharp::CSharp);

        register(&crate::swift::Swift);

        register(&crate::php::Php);

        register(&crate::dockerfile::Dockerfile);

        register(&crate::c::C);

        register(&crate::cpp::Cpp);

        register(&crate::ruby::Ruby);

        register(&crate::scala::Scala);

        register(&crate::vue::Vue);

        register(&crate::markdown::Markdown);

        register(&crate::json::Json);

        register(&crate::yaml::Yaml);

        register(&crate::toml::Toml);

        register(&crate::html::Html);

        register(&crate::css::Css);

        register(&crate::bash::Bash);

        register(&crate::lua::Lua);

        register(&crate::zig::Zig);

        register(&crate::elixir::Elixir);

        register(&crate::erlang::Erlang);

        register(&crate::dart::Dart);

        register(&crate::fsharp::FSharp);

        register(&crate::sql::Sql);

        register(&crate::graphql::GraphQL);

        register(&crate::hcl::Hcl);

        register(&crate::scss::Scss);

        register(&crate::svelte::Svelte);

        register(&crate::xml::Xml);

        register(&crate::clojure::Clojure);

        register(&crate::haskell::Haskell);

        register(&crate::ocaml::OCaml);

        register(&crate::nix::Nix);

        register(&crate::perl::Perl);

        register(&crate::r::R);

        register(&crate::julia::Julia);

        register(&crate::elm::Elm);

        register(&crate::cmake::CMake);

        register(&crate::vim::Vim);

        register(&crate::awk::Awk);

        register(&crate::fish::Fish);

        register(&crate::jq::Jq);

        register(&crate::powershell::PowerShell);

        register(&crate::zsh::Zsh);

        register(&crate::groovy::Groovy);

        register(&crate::glsl::Glsl);

        register(&crate::hlsl::Hlsl);

        register(&crate::commonlisp::CommonLisp);

        register(&crate::elisp::Elisp);

        register(&crate::gleam::Gleam);

        register(&crate::scheme::Scheme);

        register(&crate::ini::Ini);

        register(&crate::diff::Diff);

        register(&crate::dot::Dot);

        register(&crate::kdl::Kdl);

        register(&crate::ada::Ada);

        register(&crate::agda::Agda);

        register(&crate::d::D);

        register(&crate::matlab::Matlab);

        register(&crate::meson::Meson);

        register(&crate::nginx::Nginx);

        register(&crate::prolog::Prolog);

        register(&crate::batch::Batch);

        register(&crate::asm::Asm);

        register(&crate::objc::ObjC);

        register(&crate::typst::Typst);

        register(&crate::asciidoc::AsciiDoc);

        register(&crate::vb::VB);

        register(&crate::idris::Idris);

        register(&crate::rescript::ReScript);

        register(&crate::lean::Lean);

        register(&crate::caddy::Caddy);

        register(&crate::capnp::Capnp);

        register(&crate::devicetree::DeviceTree);

        register(&crate::jinja2::Jinja2);

        register(&crate::ninja::Ninja);

        register(&crate::postscript::PostScript);

        register(&crate::query::Query);

        register(&crate::ron::Ron);

        register(&crate::sparql::Sparql);

        register(&crate::sshconfig::SshConfig);

        register(&crate::starlark::Starlark);

        register(&crate::textproto::TextProto);

        register(&crate::thrift::Thrift);

        register(&crate::tlaplus::TlaPlus);

        register(&crate::uiua::Uiua);

        register(&crate::verilog::Verilog);

        register(&crate::vhdl::Vhdl);

        register(&crate::wit::Wit);

        register(&crate::x86asm::X86Asm);

        register(&crate::yuri::Yuri);
    });
}

fn extension_map() -> &'static HashMap<&'static str, &'static dyn Language> {
    init_builtin();
    EXTENSION_MAP.get_or_init(|| {
        let mut map = HashMap::new();
        let langs = LANGUAGES.read().unwrap();
        for lang in langs.iter() {
            for ext in lang.extensions() {
                map.insert(*ext, *lang);
            }
        }
        map
    })
}

fn grammar_map() -> &'static HashMap<&'static str, &'static dyn Language> {
    init_builtin();
    GRAMMAR_MAP.get_or_init(|| {
        let mut map = HashMap::new();
        let langs = LANGUAGES.read().unwrap();
        for lang in langs.iter() {
            map.insert(lang.grammar_name(), *lang);
        }
        map
    })
}

/// Get language support for a file extension.
///
/// Returns `None` if the extension is not recognized or the feature is not enabled.
pub fn support_for_extension(ext: &str) -> Option<&'static dyn Language> {
    extension_map()
        .get(ext)
        .or_else(|| extension_map().get(ext.to_lowercase().as_str()))
        .copied()
}

/// Get language support by grammar name.
///
/// Returns `None` if the grammar is not recognized or the feature is not enabled.
pub fn support_for_grammar(grammar: &str) -> Option<&'static dyn Language> {
    grammar_map().get(grammar).copied()
}

/// Get language support from a file path.
///
/// Returns `None` if the file has no extension, the extension is not recognized,
/// or the feature is not enabled.
pub fn support_for_path(path: &Path) -> Option<&'static dyn Language> {
    path.extension()
        .and_then(|e| e.to_str())
        .and_then(support_for_extension)
}

/// Get all supported languages.
pub fn supported_languages() -> Vec<&'static dyn Language> {
    init_builtin();
    LANGUAGES.read().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrammarLoader;

    /// Dump all valid node kinds for a grammar (useful for fixing invalid kinds).
    /// Run with: cargo test -p rhizome-moss-languages dump_node_kinds -- --nocapture
    #[test]
    #[ignore]
    fn dump_node_kinds() {
        let loader = GrammarLoader::new();
        // Change this to the grammar you want to inspect
        let grammar_name = std::env::var("DUMP_GRAMMAR").unwrap_or_else(|_| "python".to_string());

        let ts_lang = loader.get(&grammar_name).expect("grammar not found");

        println!("\n=== Valid node kinds for '{}' ===\n", grammar_name);
        let count = ts_lang.node_kind_count();
        for id in 0..count as u16 {
            if let Some(kind) = ts_lang.node_kind_for_id(id) {
                let named = ts_lang.node_kind_is_named(id);
                if named && !kind.starts_with('_') {
                    println!("{}", kind);
                }
            }
        }
    }

    /// Validate that all node kinds returned by Language trait methods
    /// actually exist in the tree-sitter grammar.
    #[test]
    fn validate_node_kinds() {
        let loader = GrammarLoader::new();
        let mut errors: Vec<String> = Vec::new();

        for lang in supported_languages() {
            let grammar_name = lang.grammar_name();
            let ts_lang = match loader.get(grammar_name) {
                Some(l) => l,
                None => {
                    // Grammar not available in search paths
                    continue;
                }
            };

            // Collect all node kinds from trait methods
            let all_kinds: Vec<(&str, &[&str])> = vec![
                ("container_kinds", lang.container_kinds()),
                ("function_kinds", lang.function_kinds()),
                ("type_kinds", lang.type_kinds()),
                ("import_kinds", lang.import_kinds()),
                ("public_symbol_kinds", lang.public_symbol_kinds()),
                ("scope_creating_kinds", lang.scope_creating_kinds()),
                ("control_flow_kinds", lang.control_flow_kinds()),
                ("complexity_nodes", lang.complexity_nodes()),
                ("nesting_nodes", lang.nesting_nodes()),
            ];

            for (method, kinds) in all_kinds {
                for kind in kinds {
                    // id_for_node_kind returns 0 if the kind doesn't exist
                    let id = ts_lang.id_for_node_kind(kind, true);
                    if id == 0 {
                        // Also check unnamed nodes (like operators)
                        let unnamed_id = ts_lang.id_for_node_kind(kind, false);
                        if unnamed_id == 0 {
                            errors.push(format!(
                                "{}: {}() contains invalid node kind '{}'",
                                lang.name(),
                                method,
                                kind
                            ));
                        }
                    }
                }
            }
        }

        if !errors.is_empty() {
            panic!(
                "Found {} invalid node kinds:\n{}",
                errors.len(),
                errors.join("\n")
            );
        }
    }

    /// Cross-check grammar node kinds against Language implementations.
    /// Finds potentially useful kinds that exist in the grammar but aren't used.
    /// Run with: cargo test -p rhizome-moss-languages cross_check_node_kinds -- --nocapture --ignored
    #[test]
    #[ignore]
    fn cross_check_node_kinds() {
        use std::collections::HashSet;

        let loader = GrammarLoader::new();

        // Keywords that suggest a node kind might be useful
        let interesting_patterns = [
            "statement",
            "expression",
            "definition",
            "declaration",
            "clause",
            "block",
            "body",
            "import",
            "export",
            "function",
            "method",
            "class",
            "struct",
            "enum",
            "interface",
            "trait",
            "module",
            "type",
            "return",
            "if",
            "else",
            "for",
            "while",
            "loop",
            "match",
            "case",
            "try",
            "catch",
            "except",
            "throw",
            "raise",
            "with",
            "async",
            "await",
            "yield",
            "lambda",
            "comprehension",
            "generator",
            "operator",
        ];

        for lang in supported_languages() {
            let grammar_name = lang.grammar_name();
            let ts_lang = match loader.get(grammar_name) {
                Some(l) => l,
                None => continue,
            };

            // Collect all kinds currently used by the language
            let mut used_kinds: HashSet<&str> = HashSet::new();
            for kind in lang.container_kinds() {
                used_kinds.insert(kind);
            }
            for kind in lang.function_kinds() {
                used_kinds.insert(kind);
            }
            for kind in lang.type_kinds() {
                used_kinds.insert(kind);
            }
            for kind in lang.import_kinds() {
                used_kinds.insert(kind);
            }
            for kind in lang.public_symbol_kinds() {
                used_kinds.insert(kind);
            }
            for kind in lang.scope_creating_kinds() {
                used_kinds.insert(kind);
            }
            for kind in lang.control_flow_kinds() {
                used_kinds.insert(kind);
            }
            for kind in lang.complexity_nodes() {
                used_kinds.insert(kind);
            }
            for kind in lang.nesting_nodes() {
                used_kinds.insert(kind);
            }

            // Get all valid named node kinds from grammar
            let mut all_kinds: Vec<&str> = Vec::new();
            let count = ts_lang.node_kind_count();
            for id in 0..count as u16 {
                if let Some(kind) = ts_lang.node_kind_for_id(id) {
                    let named = ts_lang.node_kind_is_named(id);
                    if named && !kind.starts_with('_') {
                        all_kinds.push(kind);
                    }
                }
            }

            // Find unused but potentially interesting kinds
            let mut unused_interesting: Vec<&str> = all_kinds
                .into_iter()
                .filter(|kind| !used_kinds.contains(*kind))
                .filter(|kind| {
                    let lower = kind.to_lowercase();
                    interesting_patterns.iter().any(|p| lower.contains(p))
                })
                .collect();

            unused_interesting.sort();

            if !unused_interesting.is_empty() {
                println!(
                    "\n=== {} ({}) - {} potentially useful unused kinds ===",
                    lang.name(),
                    grammar_name,
                    unused_interesting.len()
                );
                for kind in &unused_interesting {
                    println!("  {}", kind);
                }
            }
        }
    }
}

/// Validate that a language's unused node kinds audit is complete and accurate.
///
/// This function checks:
/// 1. All kinds in `documented_unused` actually exist in the grammar
/// 2. All potentially useful kinds from the grammar are either used or documented
///
/// Call this from each language's `unused_node_kinds_audit` test.
pub fn validate_unused_kinds_audit(
    lang: &dyn Language,
    documented_unused: &[&str],
) -> Result<(), String> {
    use crate::GrammarLoader;
    use std::collections::HashSet;

    let loader = GrammarLoader::new();
    let ts_lang = loader
        .get(lang.grammar_name())
        .ok_or_else(|| format!("Grammar '{}' not found", lang.grammar_name()))?;

    // Keywords that suggest a node kind might be useful (same as cross_check_node_kinds)
    let interesting_patterns = [
        "statement",
        "expression",
        "definition",
        "declaration",
        "clause",
        "block",
        "body",
        "import",
        "export",
        "function",
        "method",
        "class",
        "struct",
        "enum",
        "interface",
        "trait",
        "module",
        "type",
        "return",
        "if",
        "else",
        "for",
        "while",
        "loop",
        "match",
        "case",
        "try",
        "catch",
        "except",
        "throw",
        "raise",
        "with",
        "async",
        "await",
        "yield",
        "lambda",
        "comprehension",
        "generator",
        "operator",
    ];

    // Collect all kinds used by Language trait methods
    let mut used_kinds: HashSet<&str> = HashSet::new();
    for kind in lang.container_kinds() {
        used_kinds.insert(kind);
    }
    for kind in lang.function_kinds() {
        used_kinds.insert(kind);
    }
    for kind in lang.type_kinds() {
        used_kinds.insert(kind);
    }
    for kind in lang.import_kinds() {
        used_kinds.insert(kind);
    }
    for kind in lang.public_symbol_kinds() {
        used_kinds.insert(kind);
    }
    for kind in lang.scope_creating_kinds() {
        used_kinds.insert(kind);
    }
    for kind in lang.control_flow_kinds() {
        used_kinds.insert(kind);
    }
    for kind in lang.complexity_nodes() {
        used_kinds.insert(kind);
    }
    for kind in lang.nesting_nodes() {
        used_kinds.insert(kind);
    }

    let documented_set: HashSet<&str> = documented_unused.iter().copied().collect();

    // Get all valid named node kinds from grammar
    let mut grammar_kinds: HashSet<&str> = HashSet::new();
    let count = ts_lang.node_kind_count();
    for id in 0..count as u16 {
        if let Some(kind) = ts_lang.node_kind_for_id(id) {
            let named = ts_lang.node_kind_is_named(id);
            if named && !kind.starts_with('_') {
                grammar_kinds.insert(kind);
            }
        }
    }

    let mut errors: Vec<String> = Vec::new();

    // Check 1: All documented unused kinds must exist in grammar
    for kind in documented_unused {
        if !grammar_kinds.contains(*kind) {
            errors.push(format!(
                "Documented kind '{}' doesn't exist in grammar",
                kind
            ));
        }
        // Also check it's not actually being used
        if used_kinds.contains(*kind) {
            errors.push(format!(
                "Documented kind '{}' is actually used in trait methods",
                kind
            ));
        }
    }

    // Check 2: All potentially useful grammar kinds must be used or documented
    for kind in &grammar_kinds {
        let lower = kind.to_lowercase();
        let is_interesting = interesting_patterns.iter().any(|p| lower.contains(p));

        if is_interesting && !used_kinds.contains(*kind) && !documented_set.contains(*kind) {
            errors.push(format!(
                "Potentially useful kind '{}' is neither used nor documented",
                kind
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} validation errors:\n  - {}",
            errors.len(),
            errors.join("\n  - ")
        ))
    }
}
