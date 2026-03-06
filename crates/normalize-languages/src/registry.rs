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
    // normalize-syntax-allow: rust/unwrap-in-impl - RwLock poison means programmer error; recovery is not meaningful
    LANGUAGES.write().unwrap().push(lang);
}

/// Initialize built-in languages (called once).
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        #[cfg(feature = "lang-python")]
        register(&crate::python::Python);
        #[cfg(feature = "lang-rust")]
        register(&crate::rust::Rust);
        #[cfg(feature = "lang-javascript")]
        register(&crate::javascript::JavaScript);
        #[cfg(feature = "lang-typescript")]
        {
            register(&crate::typescript::TypeScript);
            register(&crate::typescript::Tsx);
        }
        #[cfg(feature = "lang-go")]
        register(&crate::go::Go);
        #[cfg(feature = "lang-java")]
        register(&crate::java::Java);
        #[cfg(feature = "lang-kotlin")]
        register(&crate::kotlin::Kotlin);
        #[cfg(feature = "lang-csharp")]
        register(&crate::csharp::CSharp);
        #[cfg(feature = "lang-swift")]
        register(&crate::swift::Swift);
        #[cfg(feature = "lang-php")]
        register(&crate::php::Php);
        #[cfg(feature = "lang-dockerfile")]
        register(&crate::dockerfile::Dockerfile);
        #[cfg(feature = "lang-c")]
        register(&crate::c::C);
        #[cfg(feature = "lang-cpp")]
        register(&crate::cpp::Cpp);
        #[cfg(feature = "lang-ruby")]
        register(&crate::ruby::Ruby);
        #[cfg(feature = "lang-scala")]
        register(&crate::scala::Scala);
        #[cfg(feature = "lang-vue")]
        register(&crate::vue::Vue);
        #[cfg(feature = "lang-markdown")]
        register(&crate::markdown::Markdown);
        #[cfg(feature = "lang-json")]
        register(&crate::json::Json);
        #[cfg(feature = "lang-yaml")]
        register(&crate::yaml::Yaml);
        #[cfg(feature = "lang-toml")]
        register(&crate::toml::Toml);
        #[cfg(feature = "lang-html")]
        register(&crate::html::Html);
        #[cfg(feature = "lang-css")]
        register(&crate::css::Css);
        #[cfg(feature = "lang-bash")]
        register(&crate::bash::Bash);
        #[cfg(feature = "lang-lua")]
        register(&crate::lua::Lua);
        #[cfg(feature = "lang-zig")]
        register(&crate::zig::Zig);
        #[cfg(feature = "lang-elixir")]
        register(&crate::elixir::Elixir);
        #[cfg(feature = "lang-erlang")]
        register(&crate::erlang::Erlang);
        #[cfg(feature = "lang-dart")]
        register(&crate::dart::Dart);
        #[cfg(feature = "lang-fsharp")]
        register(&crate::fsharp::FSharp);
        #[cfg(feature = "lang-sql")]
        register(&crate::sql::Sql);
        #[cfg(feature = "lang-graphql")]
        register(&crate::graphql::GraphQL);
        #[cfg(feature = "lang-hcl")]
        register(&crate::hcl::Hcl);
        #[cfg(feature = "lang-scss")]
        register(&crate::scss::Scss);
        #[cfg(feature = "lang-svelte")]
        register(&crate::svelte::Svelte);
        #[cfg(feature = "lang-xml")]
        register(&crate::xml::Xml);
        #[cfg(feature = "lang-clojure")]
        register(&crate::clojure::Clojure);
        #[cfg(feature = "lang-haskell")]
        register(&crate::haskell::Haskell);
        #[cfg(feature = "lang-ocaml")]
        register(&crate::ocaml::OCaml);
        #[cfg(feature = "lang-nix")]
        register(&crate::nix::Nix);
        #[cfg(feature = "lang-perl")]
        register(&crate::perl::Perl);
        #[cfg(feature = "lang-r")]
        register(&crate::r::R);
        #[cfg(feature = "lang-julia")]
        register(&crate::julia::Julia);
        #[cfg(feature = "lang-elm")]
        register(&crate::elm::Elm);
        #[cfg(feature = "lang-cmake")]
        register(&crate::cmake::CMake);
        #[cfg(feature = "lang-vim")]
        register(&crate::vim::Vim);
        #[cfg(feature = "lang-awk")]
        register(&crate::awk::Awk);
        #[cfg(feature = "lang-fish")]
        register(&crate::fish::Fish);
        #[cfg(feature = "lang-jq")]
        register(&crate::jq::Jq);
        #[cfg(feature = "lang-powershell")]
        register(&crate::powershell::PowerShell);
        #[cfg(feature = "lang-zsh")]
        register(&crate::zsh::Zsh);
        #[cfg(feature = "lang-groovy")]
        register(&crate::groovy::Groovy);
        #[cfg(feature = "lang-glsl")]
        register(&crate::glsl::Glsl);
        #[cfg(feature = "lang-hlsl")]
        register(&crate::hlsl::Hlsl);
        #[cfg(feature = "lang-commonlisp")]
        register(&crate::commonlisp::CommonLisp);
        #[cfg(feature = "lang-elisp")]
        register(&crate::elisp::Elisp);
        #[cfg(feature = "lang-gleam")]
        register(&crate::gleam::Gleam);
        #[cfg(feature = "lang-ini")]
        register(&crate::ini::Ini);
        #[cfg(feature = "lang-diff")]
        register(&crate::diff::Diff);
        #[cfg(feature = "lang-dot")]
        register(&crate::dot::Dot);
        #[cfg(feature = "lang-kdl")]
        register(&crate::kdl::Kdl);
        #[cfg(feature = "lang-ada")]
        register(&crate::ada::Ada);
        #[cfg(feature = "lang-agda")]
        register(&crate::agda::Agda);
        #[cfg(feature = "lang-d")]
        register(&crate::d::D);
        #[cfg(feature = "lang-matlab")]
        register(&crate::matlab::Matlab);
        #[cfg(feature = "lang-meson")]
        register(&crate::meson::Meson);
        #[cfg(feature = "lang-nginx")]
        register(&crate::nginx::Nginx);
        #[cfg(feature = "lang-prolog")]
        register(&crate::prolog::Prolog);
        #[cfg(feature = "lang-batch")]
        register(&crate::batch::Batch);
        #[cfg(feature = "lang-asm")]
        register(&crate::asm::Asm);
        #[cfg(feature = "lang-objc")]
        register(&crate::objc::ObjC);
        #[cfg(feature = "lang-typst")]
        register(&crate::typst::Typst);
        #[cfg(feature = "lang-asciidoc")]
        register(&crate::asciidoc::AsciiDoc);
        #[cfg(feature = "lang-vb")]
        register(&crate::vb::VB);
        #[cfg(feature = "lang-idris")]
        register(&crate::idris::Idris);
        #[cfg(feature = "lang-rescript")]
        register(&crate::rescript::ReScript);
        #[cfg(feature = "lang-lean")]
        register(&crate::lean::Lean);
        #[cfg(feature = "lang-caddy")]
        register(&crate::caddy::Caddy);
        #[cfg(feature = "lang-capnp")]
        register(&crate::capnp::Capnp);
        #[cfg(feature = "lang-devicetree")]
        register(&crate::devicetree::DeviceTree);
        #[cfg(feature = "lang-jinja2")]
        register(&crate::jinja2::Jinja2);
        #[cfg(feature = "lang-ninja")]
        register(&crate::ninja::Ninja);
        #[cfg(feature = "lang-postscript")]
        register(&crate::postscript::PostScript);
        #[cfg(feature = "lang-query")]
        register(&crate::query::Query);
        // Scheme registered after Query so .scm → Scheme (not Query) in extension_map
        #[cfg(feature = "lang-scheme")]
        register(&crate::scheme::Scheme);
        #[cfg(feature = "lang-ron")]
        register(&crate::ron::Ron);
        #[cfg(feature = "lang-sparql")]
        register(&crate::sparql::Sparql);
        #[cfg(feature = "lang-sshconfig")]
        register(&crate::sshconfig::SshConfig);
        #[cfg(feature = "lang-starlark")]
        register(&crate::starlark::Starlark);
        #[cfg(feature = "lang-textproto")]
        register(&crate::textproto::TextProto);
        #[cfg(feature = "lang-thrift")]
        register(&crate::thrift::Thrift);
        #[cfg(feature = "lang-tlaplus")]
        register(&crate::tlaplus::TlaPlus);
        #[cfg(feature = "lang-uiua")]
        register(&crate::uiua::Uiua);
        #[cfg(feature = "lang-verilog")]
        register(&crate::verilog::Verilog);
        #[cfg(feature = "lang-vhdl")]
        register(&crate::vhdl::Vhdl);
        #[cfg(feature = "lang-wit")]
        register(&crate::wit::Wit);
        #[cfg(feature = "lang-x86asm")]
        register(&crate::x86asm::X86Asm);
        #[cfg(feature = "lang-yuri")]
        register(&crate::yuri::Yuri);
    });
}

fn extension_map() -> &'static HashMap<&'static str, &'static dyn Language> {
    init_builtin();
    EXTENSION_MAP.get_or_init(|| {
        let mut map = HashMap::new();
        // normalize-syntax-allow: rust/unwrap-in-impl - RwLock poison means programmer error; recovery is not meaningful
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
        // normalize-syntax-allow: rust/unwrap-in-impl - RwLock poison means programmer error; recovery is not meaningful
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

/// Check if a file path is a dedicated test file for its language.
///
/// Returns false for unknown file types or languages that use inline tests.
/// Matches against the language's `test_file_globs()` patterns.
pub fn is_test_path(path: &Path) -> bool {
    let lang = match support_for_path(path) {
        Some(l) => l,
        None => return false,
    };
    let globs = lang.test_file_globs();
    if globs.is_empty() {
        return false;
    }
    let mut builder = globset::GlobSetBuilder::new();
    for g in globs {
        if let Ok(glob) = globset::Glob::new(g) {
            builder.add(glob);
        }
    }
    let Ok(set) = builder.build() else {
        return false;
    };
    set.is_match(path)
}

/// Get all glob patterns that identify test files for a given language extension.
pub fn test_file_globs_for_path(path: &Path) -> &'static [&'static str] {
    support_for_path(path)
        .map(|lang| lang.test_file_globs())
        .unwrap_or(&[])
}

/// Get all supported languages.
pub fn supported_languages() -> Vec<&'static dyn Language> {
    init_builtin();
    // normalize-syntax-allow: rust/unwrap-in-impl - RwLock poison means programmer error; recovery is not meaningful
    LANGUAGES.read().unwrap().clone()
}

/// Check if a path is a programming language (not a data/config format).
///
/// Returns false for data formats like JSON, YAML, TOML, Markdown, etc.
/// even though normalize-languages can parse them for syntax highlighting.
///
/// Useful for architecture analysis where only "code" files are relevant.
/// Uses `normalize_language_meta::capabilities_for()` to determine if a
/// language is executable code.
pub fn is_programming_language(path: &Path) -> bool {
    let lang = match support_for_path(path) {
        Some(l) => l,
        None => return false,
    };

    let caps = normalize_language_meta::capabilities_for(lang.name());
    caps.executable
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
    let used_kinds: HashSet<&str> = HashSet::new();

    // Also collect kinds referenced in tags.scm (these replace container/function/type_kinds)
    let tags_kinds: HashSet<String> = {
        let mut kinds = HashSet::new();
        if let Some(tags_content) = loader.get_tags(lang.grammar_name()) {
            // Extract top-level node kind names: lines starting with "(<identifier>"
            // These are the patterns like "(function_definition ..." in the query
            for line in tags_content.lines() {
                let trimmed = line.trim_start();
                if trimmed.starts_with('(')
                    && !trimmed.starts_with(";;")
                    && !trimmed.starts_with(";")
                {
                    // Extract the first word after the opening paren
                    let inner = &trimmed[1..];
                    let kind_name: String = inner
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                        .collect();
                    if !kind_name.is_empty() && !kind_name.starts_with('@') {
                        kinds.insert(kind_name);
                    }
                }
            }
        }
        kinds
    };

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
        // Also check it's not actually being used (in trait methods or tags.scm)
        if used_kinds.contains(*kind) || tags_kinds.contains(*kind) {
            errors.push(format!(
                "Documented kind '{}' is actually used in trait methods or tags.scm",
                kind
            ));
        }
    }

    // Check 2: All potentially useful grammar kinds must be used or documented
    for kind in &grammar_kinds {
        let lower = kind.to_lowercase();
        let is_interesting = interesting_patterns.iter().any(|p| lower.contains(p));

        if is_interesting
            && !used_kinds.contains(*kind)
            && !tags_kinds.contains(*kind)
            && !documented_set.contains(*kind)
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrammarLoader;

    /// Dump all valid node kinds for a grammar (useful for fixing invalid kinds).
    /// Run with: cargo test -p rhizome-normalize-languages dump_node_kinds -- --nocapture
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
    ///
    /// No trait methods return node kind lists any more —
    /// export detection now uses tags.scm queries exclusively.
    /// This test is intentionally empty.
    #[test]
    fn validate_node_kinds() {
        // Nothing to validate — node kind lists were removed from the Language trait.
    }

    /// Cross-check grammar node kinds against Language implementations.
    /// Finds potentially useful kinds that exist in the grammar but aren't used.
    /// Run with: cargo test -p rhizome-normalize-languages cross_check_node_kinds -- --nocapture --ignored
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
            // public_symbol_kinds() removed — export detection uses tags.scm exclusively.
            let used_kinds: HashSet<&str> = HashSet::new();

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
