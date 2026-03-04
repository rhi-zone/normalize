//! Dynamic grammar loading for tree-sitter.
//!
//! Loads tree-sitter grammars from shared libraries (.so/.dylib/.dll).
//! Also loads highlight queries (.scm files) for syntax highlighting.
//! Grammars are compiled from arborium sources via `cargo xtask build-grammars`.
//!
//! # ABI Compatibility
//!
//! Tree-sitter grammars have an ABI version embedded at compile time. The tree-sitter
//! library only loads grammars within its supported version range:
//! - tree-sitter 0.24: ABI 13-14
//! - tree-sitter 0.25+: ABI 13-15
//!
//! Arborium grammar crates embed the ABI version in their parser.c source. When arborium
//! updates to use newer tree-sitter, grammars must be recompiled. Stale grammars in
//! `~/.config/normalize/grammars/` may cause `LanguageError { version: N }` if incompatible.
//!
//! # Lifetime Requirements
//!
//! **IMPORTANT**: The `GrammarLoader` must outlive any `Language` or `Tree` obtained from it.
//! The loader holds the shared library (`Library`) that contains the grammar's code. If the
//! loader is dropped, the library is unloaded, and any `Language`/`Tree` references become
//! dangling pointers (use-after-free, likely segfault).
//!
//! Safe patterns:
//! - Use the global singleton (see [`crate::parsers::grammar_loader()`])
//! - Keep the loader in scope for the duration of tree usage
//! - Return `(Tree, GrammarLoader)` tuples from helper functions
//!
//! Unsafe pattern (causes segfault):
//! ```ignore
//! fn parse(code: &str) -> Tree {
//!     let loader = GrammarLoader::new();  // Created here
//!     let lang = loader.get("python").unwrap();
//!     let mut parser = Parser::new();
//!     parser.set_language(&lang).unwrap();
//!     parser.parse(code, None).unwrap()   // Tree returned
//! }  // loader dropped here - library unloaded!
//! // Tree now has dangling pointers -> segfault on use
//! ```

use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tree_sitter::Language;
use tree_sitter_language::LanguageFn;

/// Loaded grammar with its backing library.
///
/// The `_library` field keeps the shared library loaded in memory. The `language`
/// field contains pointers into this library's memory. Dropping the library while
/// the language is in use causes undefined behavior (typically segfault).
struct LoadedGrammar {
    /// Backing shared library - must outlive any use of `language`.
    _library: Library,
    /// Tree-sitter Language (contains pointers into `_library`).
    language: Language,
}

/// Dynamic grammar loader with caching.
pub struct GrammarLoader {
    /// Search paths for grammar libraries.
    search_paths: Vec<PathBuf>,
    /// Cached loaded grammars.
    cache: RwLock<HashMap<String, Arc<LoadedGrammar>>>,
    /// Cached highlight queries.
    highlight_cache: RwLock<HashMap<String, Arc<String>>>,
    /// Cached injection queries.
    injection_cache: RwLock<HashMap<String, Arc<String>>>,
    /// Cached locals queries.
    locals_cache: RwLock<HashMap<String, Arc<String>>>,
    /// Cached complexity queries.
    complexity_cache: RwLock<HashMap<String, Arc<String>>>,
    /// Cached calls queries.
    calls_cache: RwLock<HashMap<String, Arc<String>>>,
    /// Cached type queries.
    types_cache: RwLock<HashMap<String, Arc<String>>>,
    /// Cached tags queries.
    tags_cache: RwLock<HashMap<String, Arc<String>>>,
    /// Cached imports queries.
    imports_cache: RwLock<HashMap<String, Arc<String>>>,
}

impl GrammarLoader {
    /// Create a new grammar loader with default search paths.
    ///
    /// Search order:
    /// 1. `NORMALIZE_GRAMMAR_PATH` environment variable (colon-separated)
    /// 2. `~/.config/normalize/grammars/`
    pub fn new() -> Self {
        let mut paths = Vec::new();

        // Environment variable takes priority
        if let Ok(env_path) = std::env::var("NORMALIZE_GRAMMAR_PATH") {
            for p in env_path.split(':') {
                if !p.is_empty() {
                    paths.push(PathBuf::from(p));
                }
            }
        }

        // User config directory
        if let Some(config) = dirs::config_dir() {
            paths.push(config.join("normalize/grammars"));
        }

        Self {
            search_paths: paths,
            cache: RwLock::new(HashMap::new()),
            highlight_cache: RwLock::new(HashMap::new()),
            injection_cache: RwLock::new(HashMap::new()),
            locals_cache: RwLock::new(HashMap::new()),
            complexity_cache: RwLock::new(HashMap::new()),
            calls_cache: RwLock::new(HashMap::new()),
            types_cache: RwLock::new(HashMap::new()),
            tags_cache: RwLock::new(HashMap::new()),
            imports_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create a loader with custom search paths.
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            search_paths: paths,
            cache: RwLock::new(HashMap::new()),
            highlight_cache: RwLock::new(HashMap::new()),
            injection_cache: RwLock::new(HashMap::new()),
            locals_cache: RwLock::new(HashMap::new()),
            complexity_cache: RwLock::new(HashMap::new()),
            calls_cache: RwLock::new(HashMap::new()),
            types_cache: RwLock::new(HashMap::new()),
            tags_cache: RwLock::new(HashMap::new()),
            imports_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Add a search path.
    pub fn add_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Get a grammar by name.
    ///
    /// Returns None if grammar not found in search paths.
    pub fn get(&self, name: &str) -> Option<Language> {
        // Check cache first
        if let Some(loaded) = self.cache.read().ok()?.get(name) {
            return Some(loaded.language.clone());
        }

        self.load_external(name)
    }

    /// Get the highlight query for a grammar.
    ///
    /// Returns None if no highlight query found for the grammar.
    /// Query files are {name}.highlights.scm in the grammar search paths.
    pub fn get_highlights(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.highlight_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        self.load_query(name, "highlights", &self.highlight_cache)
    }

    /// Get the injection query for a grammar.
    ///
    /// Returns None if no injection query found for the grammar.
    /// Query files are {name}.injections.scm in the grammar search paths.
    pub fn get_injections(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.injection_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        self.load_query(name, "injections", &self.injection_cache)
    }

    /// Get the locals query for a grammar.
    ///
    /// Returns None if no locals query found for the grammar.
    /// Query files are {name}.locals.scm in the grammar search paths.
    pub fn get_locals(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.locals_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        self.load_query(name, "locals", &self.locals_cache)
    }

    /// Get the complexity query for a grammar.
    ///
    /// Returns None if no complexity query found for the grammar.
    /// Query files are {name}.complexity.scm in the grammar search paths.
    /// Uses `@complexity` captures for nodes that increase cyclomatic complexity,
    /// and `@nesting` captures for nodes that increase nesting depth.
    pub fn get_complexity(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.complexity_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        // Try external files, then fall back to bundled queries
        self.load_query(name, "complexity", &self.complexity_cache)
            .or_else(|| {
                let content = bundled_complexity_query(name)?;
                let query = Arc::new(content.to_string());
                if let Ok(mut c) = self.complexity_cache.write() {
                    c.insert(name.to_string(), Arc::clone(&query));
                }
                Some(query)
            })
    }

    /// Get the calls query for a grammar.
    ///
    /// Returns None if no calls query found for the grammar.
    /// Query files are {name}.calls.scm in the grammar search paths.
    /// Uses `@call` captures for call expressions and `@call.qualifier` for
    /// method call receivers (e.g. `foo` in `foo.bar()`).
    pub fn get_calls(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.calls_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        // Try external files, then fall back to bundled queries
        self.load_query(name, "calls", &self.calls_cache)
            .or_else(|| {
                let content = bundled_calls_query(name)?;
                let query = Arc::new(content.to_string());
                if let Ok(mut c) = self.calls_cache.write() {
                    c.insert(name.to_string(), Arc::clone(&query));
                }
                Some(query)
            })
    }

    /// Get the types query for a grammar.
    ///
    /// Returns the bundled query for supported languages, or an external file if one
    /// exists at `{name}.types.scm` in the grammar search paths (external wins).
    pub fn get_types(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.types_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        // External file takes priority over bundled
        if let Some(q) = self.load_query(name, "types", &self.types_cache) {
            return Some(q);
        }

        // Fall back to bundled query
        let bundled = bundled_types_query(name)?;
        let query = Arc::new(bundled.to_string());
        if let Ok(mut c) = self.types_cache.write() {
            c.insert(name.to_string(), Arc::clone(&query));
        }
        Some(query)
    }

    /// Get the tags query for a grammar.
    ///
    /// Tags queries use the tree-sitter tags format with `@name.definition.*` and
    /// `@name.reference.*` captures for symbol navigation (used by GitHub Linguist,
    /// nvim-treesitter, etc.).
    ///
    /// Returns the bundled query for supported languages, or an external file if one
    /// exists at `{name}.tags.scm` in the grammar search paths (external wins).
    pub fn get_tags(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.tags_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        // External file takes priority over bundled
        if let Some(q) = self.load_query(name, "tags", &self.tags_cache) {
            return Some(q);
        }

        // Fall back to bundled query
        let bundled = bundled_tags_query(name)?;
        let query = Arc::new(bundled.to_string());
        if let Ok(mut c) = self.tags_cache.write() {
            c.insert(name.to_string(), Arc::clone(&query));
        }
        Some(query)
    }

    /// Get the imports query for a grammar.
    ///
    /// Returns the bundled query for supported languages, or an external file if one
    /// exists at `{name}.imports.scm` in the grammar search paths (external wins).
    pub fn get_imports(&self, name: &str) -> Option<Arc<String>> {
        // Check cache first
        if let Some(query) = self.imports_cache.read().ok()?.get(name) {
            return Some(Arc::clone(query));
        }

        // External file takes priority over bundled
        if let Some(q) = self.load_query(name, "imports", &self.imports_cache) {
            return Some(q);
        }

        // Fall back to bundled query
        let bundled = bundled_imports_query(name)?;
        let query = Arc::new(bundled.to_string());
        if let Ok(mut c) = self.imports_cache.write() {
            c.insert(name.to_string(), Arc::clone(&query));
        }
        Some(query)
    }

    /// Load a query file (.scm) from external file.
    fn load_query(
        &self,
        name: &str,
        query_type: &str,
        cache: &RwLock<HashMap<String, Arc<String>>>,
    ) -> Option<Arc<String>> {
        let scm_name = format!("{name}.{query_type}.scm");

        for search_path in &self.search_paths {
            let scm_path = search_path.join(&scm_name);
            if scm_path.exists()
                && let Ok(content) = std::fs::read_to_string(&scm_path)
            {
                let query = Arc::new(content);

                // Cache it
                if let Ok(mut c) = cache.write() {
                    c.insert(name.to_string(), Arc::clone(&query));
                }

                return Some(query);
            }
        }

        None
    }

    /// Load a grammar from external .so file.
    fn load_external(&self, name: &str) -> Option<Language> {
        let lib_name = grammar_lib_name(name);

        for search_path in &self.search_paths {
            let lib_path = search_path.join(&lib_name);
            if lib_path.exists()
                && let Some(lang) = self.load_from_path(name, &lib_path)
            {
                return Some(lang);
            }
        }

        None
    }

    /// Load grammar from a specific path.
    fn load_from_path(&self, name: &str, path: &Path) -> Option<Language> {
        // SAFETY: Loading shared libraries is inherently unsafe. We accept this risk because:
        // 1. Grammars come from arborium (bundled) or user-configured search paths
        // 2. The alternative (no dynamic loading) would require compiling all grammars statically
        // 3. Tree-sitter grammars are widely used and well-tested
        let library = unsafe { Library::new(path).ok()? };

        let symbol_name = grammar_symbol_name(name);
        // SAFETY: We call the tree-sitter grammar function which returns a Language pointer.
        // The function signature is defined by tree-sitter's C ABI. We trust that:
        // 1. The symbol exists (checked by library.get)
        // 2. The function conforms to tree-sitter's expected signature
        // 3. The returned Language is valid for the lifetime of the library
        let language = unsafe {
            let func: Symbol<unsafe extern "C" fn() -> *const ()> =
                library.get(symbol_name.as_bytes()).ok()?;
            let lang_fn = LanguageFn::from_raw(*func);
            Language::new(lang_fn)
        };

        // Cache the loaded grammar
        let loaded = Arc::new(LoadedGrammar {
            _library: library,
            language: language.clone(),
        });

        if let Ok(mut cache) = self.cache.write() {
            cache.insert(name.to_string(), loaded);
        }

        Some(language)
    }

    /// List available grammars in search paths.
    pub fn available_external(&self) -> Vec<String> {
        let mut grammars = Vec::new();
        let ext = grammar_extension();

        for path in &self.search_paths {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.ends_with(ext) {
                        let grammar_name = name_str.trim_end_matches(ext);
                        if !grammars.contains(&grammar_name.to_string()) {
                            grammars.push(grammar_name.to_string());
                        }
                    }
                }
            }
        }

        grammars.sort();
        grammars
    }
}

impl Default for GrammarLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the library file name for a grammar.
fn grammar_lib_name(name: &str) -> String {
    let ext = grammar_extension();
    format!("{name}{ext}")
}

/// Get the expected symbol name for a grammar.
fn grammar_symbol_name(name: &str) -> String {
    // Special cases for arborium grammars with non-standard symbol names
    match name {
        "rust" => return "tree_sitter_rust_orchard".to_string(),
        "vb" => return "tree_sitter_vb_dotnet".to_string(),
        _ => {}
    }
    // Most grammars use tree_sitter_{name} with hyphens replaced by underscores
    let normalized = name.replace('-', "_");
    format!("tree_sitter_{normalized}")
}

/// Return a bundled types query for languages with built-in support.
/// Returns None for languages without a bundled query.
fn bundled_types_query(name: &str) -> Option<&'static str> {
    match name {
        "rust" => Some(include_str!("queries/rust.types.scm")),
        "typescript" => Some(include_str!("queries/typescript.types.scm")),
        "tsx" => Some(include_str!("queries/tsx.types.scm")),
        "python" => Some(include_str!("queries/python.types.scm")),
        "java" => Some(include_str!("queries/java.types.scm")),
        "go" => Some(include_str!("queries/go.types.scm")),
        "c" => Some(include_str!("queries/c.types.scm")),
        "cpp" => Some(include_str!("queries/cpp.types.scm")),
        "kotlin" => Some(include_str!("queries/kotlin.types.scm")),
        "swift" => Some(include_str!("queries/swift.types.scm")),
        "c-sharp" => Some(include_str!("queries/c-sharp.types.scm")),
        "scala" => Some(include_str!("queries/scala.types.scm")),
        "haskell" => Some(include_str!("queries/haskell.types.scm")),
        "ruby" => Some(include_str!("queries/ruby.types.scm")),
        "dart" => Some(include_str!("queries/dart.types.scm")),
        "elixir" => Some(include_str!("queries/elixir.types.scm")),
        "ocaml" => Some(include_str!("queries/ocaml.types.scm")),
        "erlang" => Some(include_str!("queries/erlang.types.scm")),
        "zig" => Some(include_str!("queries/zig.types.scm")),
        "fsharp" => Some(include_str!("queries/fsharp.types.scm")),
        "gleam" => Some(include_str!("queries/gleam.types.scm")),
        "julia" => Some(include_str!("queries/julia.types.scm")),
        "r" => Some(include_str!("queries/r.types.scm")),
        "d" => Some(include_str!("queries/d.types.scm")),
        "objc" => Some(include_str!("queries/objc.types.scm")),
        "vb" => Some(include_str!("queries/vb.types.scm")),
        "groovy" => Some(include_str!("queries/groovy.types.scm")),
        _ => None,
    }
}

/// Return a bundled tags query for languages with built-in support.
///
/// Tags queries use the tree-sitter tags format (`@name.definition.*` and
/// `@name.reference.*` captures) for symbol navigation. Sources are vendored from
/// official tree-sitter grammar repositories (MIT licensed).
fn bundled_tags_query(name: &str) -> Option<&'static str> {
    match name {
        "rust" => Some(include_str!("queries/rust.tags.scm")),
        "python" => Some(include_str!("queries/python.tags.scm")),
        "javascript" => Some(include_str!("queries/javascript.tags.scm")),
        "typescript" => Some(include_str!("queries/typescript.tags.scm")),
        "tsx" => Some(include_str!("queries/tsx.tags.scm")),
        "go" => Some(include_str!("queries/go.tags.scm")),
        "java" => Some(include_str!("queries/java.tags.scm")),
        "c" => Some(include_str!("queries/c.tags.scm")),
        "cpp" => Some(include_str!("queries/cpp.tags.scm")),
        "ruby" => Some(include_str!("queries/ruby.tags.scm")),
        "kotlin" => Some(include_str!("queries/kotlin.tags.scm")),
        _ => None,
    }
}

/// Get the shared library extension for the current platform.
fn grammar_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        ".dylib"
    } else if cfg!(target_os = "windows") {
        ".dll"
    } else {
        ".so"
    }
}

/// Return a bundled complexity query for a grammar, if available.
///
/// These are compiled into the binary so they work without external .scm files.
/// External files in search paths take priority (for user customization).
fn bundled_complexity_query(name: &str) -> Option<&'static str> {
    match name {
        "rust" => Some(include_str!("queries/rust.complexity.scm")),
        "python" => Some(include_str!("queries/python.complexity.scm")),
        "go" => Some(include_str!("queries/go.complexity.scm")),
        "javascript" => Some(include_str!("queries/javascript.complexity.scm")),
        "typescript" => Some(include_str!("queries/typescript.complexity.scm")),
        "tsx" => Some(include_str!("queries/tsx.complexity.scm")),
        "java" => Some(include_str!("queries/java.complexity.scm")),
        "c" => Some(include_str!("queries/c.complexity.scm")),
        "cpp" => Some(include_str!("queries/cpp.complexity.scm")),
        "ruby" => Some(include_str!("queries/ruby.complexity.scm")),
        "kotlin" => Some(include_str!("queries/kotlin.complexity.scm")),
        "swift" => Some(include_str!("queries/swift.complexity.scm")),
        "c-sharp" => Some(include_str!("queries/c-sharp.complexity.scm")),
        "bash" => Some(include_str!("queries/bash.complexity.scm")),
        "lua" => Some(include_str!("queries/lua.complexity.scm")),
        "elixir" => Some(include_str!("queries/elixir.complexity.scm")),
        "scala" => Some(include_str!("queries/scala.complexity.scm")),
        "dart" => Some(include_str!("queries/dart.complexity.scm")),
        "zig" => Some(include_str!("queries/zig.complexity.scm")),
        "ocaml" => Some(include_str!("queries/ocaml.complexity.scm")),
        "erlang" => Some(include_str!("queries/erlang.complexity.scm")),
        "php" => Some(include_str!("queries/php.complexity.scm")),
        "haskell" => Some(include_str!("queries/haskell.complexity.scm")),
        "r" => Some(include_str!("queries/r.complexity.scm")),
        "julia" => Some(include_str!("queries/julia.complexity.scm")),
        "perl" => Some(include_str!("queries/perl.complexity.scm")),
        "groovy" => Some(include_str!("queries/groovy.complexity.scm")),
        "elm" => Some(include_str!("queries/elm.complexity.scm")),
        "powershell" => Some(include_str!("queries/powershell.complexity.scm")),
        "fish" => Some(include_str!("queries/fish.complexity.scm")),
        "fsharp" => Some(include_str!("queries/fsharp.complexity.scm")),
        "gleam" => Some(include_str!("queries/gleam.complexity.scm")),
        "clojure" => Some(include_str!("queries/clojure.complexity.scm")),
        "commonlisp" => Some(include_str!("queries/commonlisp.complexity.scm")),
        "scheme" => Some(include_str!("queries/scheme.complexity.scm")),
        "d" => Some(include_str!("queries/d.complexity.scm")),
        "objc" => Some(include_str!("queries/objc.complexity.scm")),
        "vb" => Some(include_str!("queries/vb.complexity.scm")),
        "elisp" => Some(include_str!("queries/elisp.complexity.scm")),
        "hcl" => Some(include_str!("queries/hcl.complexity.scm")),
        "matlab" => Some(include_str!("queries/matlab.complexity.scm")),
        "nix" => Some(include_str!("queries/nix.complexity.scm")),
        "sql" => Some(include_str!("queries/sql.complexity.scm")),
        "starlark" => Some(include_str!("queries/starlark.complexity.scm")),
        "vim" => Some(include_str!("queries/vim.complexity.scm")),
        "zsh" => Some(include_str!("queries/zsh.complexity.scm")),
        "rescript" => Some(include_str!("queries/rescript.complexity.scm")),
        "idris" => Some(include_str!("queries/idris.complexity.scm")),
        "lean" => Some(include_str!("queries/lean.complexity.scm")),
        "ada" => Some(include_str!("queries/ada.complexity.scm")),
        "agda" => Some(include_str!("queries/agda.complexity.scm")),
        "awk" => Some(include_str!("queries/awk.complexity.scm")),
        "cmake" => Some(include_str!("queries/cmake.complexity.scm")),
        "glsl" => Some(include_str!("queries/glsl.complexity.scm")),
        "graphql" => Some(include_str!("queries/graphql.complexity.scm")),
        "hlsl" => Some(include_str!("queries/hlsl.complexity.scm")),
        "jq" => Some(include_str!("queries/jq.complexity.scm")),
        "meson" => Some(include_str!("queries/meson.complexity.scm")),
        "nginx" => Some(include_str!("queries/nginx.complexity.scm")),
        "prolog" => Some(include_str!("queries/prolog.complexity.scm")),
        "scss" => Some(include_str!("queries/scss.complexity.scm")),
        "svelte" => Some(include_str!("queries/svelte.complexity.scm")),
        "tlaplus" => Some(include_str!("queries/tlaplus.complexity.scm")),
        "typst" => Some(include_str!("queries/typst.complexity.scm")),
        "verilog" => Some(include_str!("queries/verilog.complexity.scm")),
        "vhdl" => Some(include_str!("queries/vhdl.complexity.scm")),
        "vue" => Some(include_str!("queries/vue.complexity.scm")),
        _ => None,
    }
}

/// Return a bundled calls query for a grammar, if available.
fn bundled_calls_query(name: &str) -> Option<&'static str> {
    match name {
        "python" => Some(include_str!("queries/python.calls.scm")),
        "rust" => Some(include_str!("queries/rust.calls.scm")),
        "typescript" => Some(include_str!("queries/typescript.calls.scm")),
        "tsx" => Some(include_str!("queries/tsx.calls.scm")),
        "javascript" => Some(include_str!("queries/javascript.calls.scm")),
        "java" => Some(include_str!("queries/java.calls.scm")),
        "go" => Some(include_str!("queries/go.calls.scm")),
        "c" => Some(include_str!("queries/c.calls.scm")),
        "cpp" => Some(include_str!("queries/cpp.calls.scm")),
        "ruby" => Some(include_str!("queries/ruby.calls.scm")),
        "kotlin" => Some(include_str!("queries/kotlin.calls.scm")),
        "swift" => Some(include_str!("queries/swift.calls.scm")),
        "c-sharp" => Some(include_str!("queries/c-sharp.calls.scm")),
        "bash" => Some(include_str!("queries/bash.calls.scm")),
        "scala" => Some(include_str!("queries/scala.calls.scm")),
        "elixir" => Some(include_str!("queries/elixir.calls.scm")),
        "lua" => Some(include_str!("queries/lua.calls.scm")),
        "dart" => Some(include_str!("queries/dart.calls.scm")),
        "ocaml" => Some(include_str!("queries/ocaml.calls.scm")),
        "erlang" => Some(include_str!("queries/erlang.calls.scm")),
        "zig" => Some(include_str!("queries/zig.calls.scm")),
        "julia" => Some(include_str!("queries/julia.calls.scm")),
        "r" => Some(include_str!("queries/r.calls.scm")),
        "haskell" => Some(include_str!("queries/haskell.calls.scm")),
        "php" => Some(include_str!("queries/php.calls.scm")),
        "perl" => Some(include_str!("queries/perl.calls.scm")),
        "fsharp" => Some(include_str!("queries/fsharp.calls.scm")),
        "gleam" => Some(include_str!("queries/gleam.calls.scm")),
        "groovy" => Some(include_str!("queries/groovy.calls.scm")),
        "clojure" => Some(include_str!("queries/clojure.calls.scm")),
        "d" => Some(include_str!("queries/d.calls.scm")),
        "objc" => Some(include_str!("queries/objc.calls.scm")),
        "elisp" => Some(include_str!("queries/elisp.calls.scm")),
        "hcl" => Some(include_str!("queries/hcl.calls.scm")),
        "matlab" => Some(include_str!("queries/matlab.calls.scm")),
        "nix" => Some(include_str!("queries/nix.calls.scm")),
        "starlark" => Some(include_str!("queries/starlark.calls.scm")),
        "vim" => Some(include_str!("queries/vim.calls.scm")),
        "zsh" => Some(include_str!("queries/zsh.calls.scm")),
        "rescript" => Some(include_str!("queries/rescript.calls.scm")),
        "prolog" => Some(include_str!("queries/prolog.calls.scm")),
        "sql" => Some(include_str!("queries/sql.calls.scm")),
        _ => None,
    }
}

/// Return a bundled imports query for a grammar, if available.
fn bundled_imports_query(name: &str) -> Option<&'static str> {
    match name {
        "python" => Some(include_str!("queries/python.imports.scm")),
        "javascript" => Some(include_str!("queries/javascript.imports.scm")),
        "go" => Some(include_str!("queries/go.imports.scm")),
        "lua" => Some(include_str!("queries/lua.imports.scm")),
        "rust" => Some(include_str!("queries/rust.imports.scm")),
        "typescript" => Some(include_str!("queries/typescript.imports.scm")),
        "tsx" => Some(include_str!("queries/tsx.imports.scm")),
        "java" => Some(include_str!("queries/java.imports.scm")),
        "kotlin" => Some(include_str!("queries/kotlin.imports.scm")),
        "c-sharp" => Some(include_str!("queries/c-sharp.imports.scm")),
        "ruby" => Some(include_str!("queries/ruby.imports.scm")),
        "swift" => Some(include_str!("queries/swift.imports.scm")),
        "scala" => Some(include_str!("queries/scala.imports.scm")),
        "elixir" => Some(include_str!("queries/elixir.imports.scm")),
        "dart" => Some(include_str!("queries/dart.imports.scm")),
        "php" => Some(include_str!("queries/php.imports.scm")),
        "c" => Some(include_str!("queries/c.imports.scm")),
        "cpp" => Some(include_str!("queries/cpp.imports.scm")),
        "bash" => Some(include_str!("queries/bash.imports.scm")),
        "zsh" => Some(include_str!("queries/zsh.imports.scm")),
        "fish" => Some(include_str!("queries/fish.imports.scm")),
        "perl" => Some(include_str!("queries/perl.imports.scm")),
        "r" => Some(include_str!("queries/r.imports.scm")),
        "haskell" => Some(include_str!("queries/haskell.imports.scm")),
        "ocaml" => Some(include_str!("queries/ocaml.imports.scm")),
        "fsharp" => Some(include_str!("queries/fsharp.imports.scm")),
        "erlang" => Some(include_str!("queries/erlang.imports.scm")),
        "gleam" => Some(include_str!("queries/gleam.imports.scm")),
        "zig" => Some(include_str!("queries/zig.imports.scm")),
        "julia" => Some(include_str!("queries/julia.imports.scm")),
        "groovy" => Some(include_str!("queries/groovy.imports.scm")),
        "clojure" => Some(include_str!("queries/clojure.imports.scm")),
        "commonlisp" => Some(include_str!("queries/commonlisp.imports.scm")),
        "scheme" => Some(include_str!("queries/scheme.imports.scm")),
        "elisp" => Some(include_str!("queries/elisp.imports.scm")),
        "d" => Some(include_str!("queries/d.imports.scm")),
        "objc" => Some(include_str!("queries/objc.imports.scm")),
        "vb" => Some(include_str!("queries/vb.imports.scm")),
        "powershell" => Some(include_str!("queries/powershell.imports.scm")),
        "vim" => Some(include_str!("queries/vim.imports.scm")),
        "matlab" => Some(include_str!("queries/matlab.imports.scm")),
        "nix" => Some(include_str!("queries/nix.imports.scm")),
        "starlark" => Some(include_str!("queries/starlark.imports.scm")),
        "rescript" => Some(include_str!("queries/rescript.imports.scm")),
        "idris" => Some(include_str!("queries/idris.imports.scm")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grammar_lib_name() {
        let name = grammar_lib_name("python");
        assert!(name.starts_with("python."));
    }

    #[test]
    fn test_grammar_symbol_name() {
        assert_eq!(grammar_symbol_name("python"), "tree_sitter_python");
        assert_eq!(grammar_symbol_name("rust"), "tree_sitter_rust_orchard");
        assert_eq!(grammar_symbol_name("ssh-config"), "tree_sitter_ssh_config");
        assert_eq!(grammar_symbol_name("vb"), "tree_sitter_vb_dotnet");
    }

    #[test]
    fn test_bundled_tags_queries() {
        for lang in &[
            "rust",
            "python",
            "javascript",
            "typescript",
            "tsx",
            "go",
            "java",
            "c",
            "cpp",
            "ruby",
            "kotlin",
        ] {
            let query = bundled_tags_query(lang);
            assert!(query.is_some(), "Missing bundled tags query for {lang}");
            assert!(
                !query.unwrap().is_empty(),
                "Empty bundled tags query for {lang}"
            );
        }
    }

    #[test]
    fn test_bundled_types_queries() {
        for lang in &[
            "rust",
            "python",
            "typescript",
            "tsx",
            "java",
            "go",
            "c",
            "cpp",
            "kotlin",
            "swift",
            "c-sharp",
            "scala",
            "haskell",
            "ruby",
            "dart",
            "elixir",
            "ocaml",
            "erlang",
            "zig",
            "fsharp",
            "gleam",
            "julia",
            "r",
            "d",
            "objc",
            "vb",
            "groovy",
        ] {
            let query = bundled_types_query(lang);
            assert!(query.is_some(), "Missing bundled types query for {lang}");
            assert!(
                !query.unwrap().is_empty(),
                "Empty bundled types query for {lang}"
            );
        }
    }

    #[test]
    fn test_bundled_complexity_queries() {
        for lang in &[
            "rust",
            "python",
            "go",
            "javascript",
            "typescript",
            "tsx",
            "java",
            "c",
            "cpp",
            "ruby",
            "kotlin",
            "swift",
            "c-sharp",
            "bash",
            "lua",
            "elixir",
            "scala",
            "dart",
            "zig",
            "ocaml",
            "erlang",
            "php",
            "haskell",
            "r",
            "julia",
            "perl",
            "groovy",
            "elm",
            "powershell",
            "fish",
            "fsharp",
            "gleam",
            "clojure",
            "commonlisp",
            "scheme",
            "d",
            "objc",
            "vb",
            "elisp",
            "hcl",
            "matlab",
            "nix",
            "sql",
            "starlark",
            "vim",
            "zsh",
            "rescript",
            "idris",
            "lean",
        ] {
            let query = bundled_complexity_query(lang);
            assert!(
                query.is_some(),
                "Missing bundled complexity query for {lang}"
            );
            assert!(
                !query.unwrap().is_empty(),
                "Empty bundled complexity query for {lang}"
            );
        }
    }

    #[test]
    fn test_bundled_calls_queries() {
        for lang in &[
            "python",
            "rust",
            "typescript",
            "tsx",
            "javascript",
            "java",
            "go",
            "c",
            "cpp",
            "ruby",
            "kotlin",
            "swift",
            "c-sharp",
            "bash",
            "scala",
            "elixir",
            "lua",
            "dart",
            "ocaml",
            "erlang",
            "zig",
            "julia",
            "r",
            "haskell",
            "php",
            "perl",
            "fsharp",
            "gleam",
            "groovy",
            "clojure",
            "d",
            "objc",
            "elisp",
            "hcl",
            "matlab",
            "nix",
            "starlark",
            "vim",
            "zsh",
            "rescript",
            "prolog",
            "sql",
        ] {
            let query = bundled_calls_query(lang);
            assert!(query.is_some(), "Missing bundled calls query for {lang}");
            assert!(
                !query.unwrap().is_empty(),
                "Empty bundled calls query for {lang}"
            );
        }
    }

    #[test]
    fn test_bundled_imports_queries() {
        for lang in &[
            "python",
            "javascript",
            "go",
            "lua",
            "rust",
            "typescript",
            "tsx",
            "java",
            "kotlin",
            "c-sharp",
            "ruby",
            "swift",
            "scala",
            "elixir",
            "dart",
            "php",
            "c",
            "cpp",
            "bash",
            "zsh",
            "fish",
            "perl",
            "r",
            "haskell",
            "ocaml",
            "fsharp",
            "erlang",
            "gleam",
            "zig",
            "julia",
            "groovy",
            "clojure",
            "commonlisp",
            "scheme",
            "elisp",
            "d",
            "objc",
            "vb",
            "powershell",
            "vim",
            "matlab",
            "nix",
            "starlark",
            "rescript",
            "idris",
        ] {
            let query = bundled_imports_query(lang);
            assert!(query.is_some(), "Missing bundled imports query for {lang}");
            assert!(
                !query.unwrap().is_empty(),
                "Empty bundled imports query for {lang}"
            );
        }
    }

    #[test]
    fn test_get_imports_returns_bundled() {
        let loader = GrammarLoader::with_paths(vec![]);
        assert!(loader.get_imports("python").is_some());
        assert!(loader.get_imports("javascript").is_some());
        assert!(loader.get_imports("go").is_some());
        assert!(loader.get_imports("lua").is_some());
        assert!(loader.get_imports("rust").is_some());
        assert!(loader.get_imports("typescript").is_some());
        assert!(loader.get_imports("tsx").is_some());
        assert!(loader.get_imports("java").is_some());
        assert!(loader.get_imports("kotlin").is_some());
        assert!(loader.get_imports("c-sharp").is_some());
        assert!(loader.get_imports("ruby").is_some());
        assert!(loader.get_imports("swift").is_some());
        assert!(loader.get_imports("scala").is_some());
        assert!(loader.get_imports("elixir").is_some());
        assert!(loader.get_imports("dart").is_some());
        assert!(loader.get_imports("php").is_some());
        assert!(loader.get_imports("c").is_some());
        assert!(loader.get_imports("cpp").is_some());
        assert!(loader.get_imports("bash").is_some());
        assert!(loader.get_imports("zsh").is_some());
        assert!(loader.get_imports("fish").is_some());
        assert!(loader.get_imports("perl").is_some());
        assert!(loader.get_imports("r").is_some());
        assert!(loader.get_imports("haskell").is_some());
        assert!(loader.get_imports("ocaml").is_some());
        assert!(loader.get_imports("fsharp").is_some());
        assert!(loader.get_imports("erlang").is_some());
        assert!(loader.get_imports("gleam").is_some());
        assert!(loader.get_imports("zig").is_some());
        assert!(loader.get_imports("julia").is_some());
        assert!(loader.get_imports("groovy").is_some());
        assert!(loader.get_imports("clojure").is_some());
        assert!(loader.get_imports("commonlisp").is_some());
        assert!(loader.get_imports("scheme").is_some());
        assert!(loader.get_imports("elisp").is_some());
        assert!(loader.get_imports("d").is_some());
        assert!(loader.get_imports("objc").is_some());
        assert!(loader.get_imports("vb").is_some());
        assert!(loader.get_imports("powershell").is_some());
        assert!(loader.get_imports("vim").is_some());
        assert!(loader.get_imports("matlab").is_some());
        assert!(loader.get_imports("nix").is_some());
        assert!(loader.get_imports("starlark").is_some());
        assert!(loader.get_imports("rescript").is_some());
        assert!(loader.get_imports("idris").is_some());
        assert!(loader.get_imports("unknown-lang-xyz").is_none());
    }

    #[test]
    fn test_get_tags_returns_bundled() {
        let loader = GrammarLoader::with_paths(vec![]);
        assert!(loader.get_tags("rust").is_some());
        assert!(loader.get_tags("python").is_some());
        assert!(loader.get_tags("go").is_some());
        assert!(loader.get_tags("unknown-lang-xyz").is_none());
    }

    #[test]
    fn test_get_types_returns_bundled() {
        let loader = GrammarLoader::with_paths(vec![]);
        assert!(loader.get_types("rust").is_some());
        assert!(loader.get_types("python").is_some());
        assert!(loader.get_types("java").is_some());
        assert!(loader.get_types("go").is_some());
        assert!(loader.get_types("c").is_some());
        assert!(loader.get_types("cpp").is_some());
        assert!(loader.get_types("kotlin").is_some());
        assert!(loader.get_types("swift").is_some());
        assert!(loader.get_types("c-sharp").is_some());
        assert!(loader.get_types("unknown-lang-xyz").is_none());
    }

    #[test]
    fn test_get_calls_returns_bundled() {
        let loader = GrammarLoader::with_paths(vec![]);
        assert!(loader.get_calls("rust").is_some());
        assert!(loader.get_calls("python").is_some());
        assert!(loader.get_calls("go").is_some());
        assert!(loader.get_calls("c").is_some());
        assert!(loader.get_calls("cpp").is_some());
        assert!(loader.get_calls("ruby").is_some());
        assert!(loader.get_calls("kotlin").is_some());
        assert!(loader.get_calls("swift").is_some());
        assert!(loader.get_calls("c-sharp").is_some());
        assert!(loader.get_calls("bash").is_some());
        assert!(loader.get_calls("unknown-lang-xyz").is_none());
    }

    #[test]
    fn test_load_from_env() {
        // Set up env var pointing to target/grammars
        let grammar_path = std::env::current_dir().unwrap().join("target/grammars");

        if !grammar_path.exists() {
            eprintln!("Skipping: run `cargo xtask build-grammars` first");
            return;
        }

        // SAFETY: This is a test that runs single-threaded
        unsafe {
            std::env::set_var("NORMALIZE_GRAMMAR_PATH", grammar_path.to_str().unwrap());
        }

        let loader = GrammarLoader::new();

        // Should load python from .so
        let ext = grammar_extension();
        if grammar_path.join(format!("python{ext}")).exists() {
            let lang = loader.get("python");
            assert!(lang.is_some(), "Failed to load python grammar");
        }

        // Clean up
        // SAFETY: This is a test that runs single-threaded
        unsafe {
            std::env::remove_var("NORMALIZE_GRAMMAR_PATH");
        }
    }
}
