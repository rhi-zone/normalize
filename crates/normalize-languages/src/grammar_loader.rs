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
//! `~/.config/moss/grammars/` may cause `LanguageError { version: N }` if incompatible.
//!
//! # Lifetime Requirements
//!
//! **IMPORTANT**: The `GrammarLoader` must outlive any `Language` or `Tree` obtained from it.
//! The loader holds the shared library (`Library`) that contains the grammar's code. If the
//! loader is dropped, the library is unloaded, and any `Language`/`Tree` references become
//! dangling pointers (use-after-free, likely segfault).
//!
//! Safe patterns:
//! - Use a global singleton loader (see `normalize::parsers::grammar_loader()`)
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
}

impl GrammarLoader {
    /// Create a new grammar loader with default search paths.
    ///
    /// Search order:
    /// 1. `MOSS_GRAMMAR_PATH` environment variable (colon-separated)
    /// 2. `~/.config/moss/grammars/`
    pub fn new() -> Self {
        let mut paths = Vec::new();

        // Environment variable takes priority
        if let Ok(env_path) = std::env::var("MOSS_GRAMMAR_PATH") {
            for p in env_path.split(':') {
                if !p.is_empty() {
                    paths.push(PathBuf::from(p));
                }
            }
        }

        // User config directory
        if let Some(config) = dirs::config_dir() {
            paths.push(config.join("moss/grammars"));
        }

        Self {
            search_paths: paths,
            cache: RwLock::new(HashMap::new()),
            highlight_cache: RwLock::new(HashMap::new()),
            injection_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create a loader with custom search paths.
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            search_paths: paths,
            cache: RwLock::new(HashMap::new()),
            highlight_cache: RwLock::new(HashMap::new()),
            injection_cache: RwLock::new(HashMap::new()),
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
    fn test_load_from_env() {
        // Set up env var pointing to target/grammars
        let grammar_path = std::env::current_dir().unwrap().join("target/grammars");

        if !grammar_path.exists() {
            eprintln!("Skipping: run `cargo xtask build-grammars` first");
            return;
        }

        // SAFETY: This is a test that runs single-threaded
        unsafe {
            std::env::set_var("MOSS_GRAMMAR_PATH", grammar_path.to_str().unwrap());
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
            std::env::remove_var("MOSS_GRAMMAR_PATH");
        }
    }
}
