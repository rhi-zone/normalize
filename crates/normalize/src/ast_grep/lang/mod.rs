#![allow(warnings, clippy::all)]
// Replacement for ast-grep's SgLang type.
// Uses normalize-languages' dynamic grammar loading instead of ast-grep-language's
// embedded grammars, avoiding grammar duplication.

mod lang_globs;

use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage, TSRange};
use ast_grep_core::{Language as AstGrepLanguage, Node};
use ignore::types::Types;
use normalize_languages::GrammarLoader;
use normalize_languages::ast_grep::DynLang;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::str::FromStr;
use std::sync::OnceLock;

pub use lang_globs::LanguageGlobs;

/// A dynamically loaded language for the ast-grep CLI.
///
/// Replaces upstream's `SgLang` type. Instead of embedding grammars
/// via `ast-grep-language`, uses normalize-languages' `GrammarLoader`
/// which dynamically loads tree-sitter grammars from normalize-grammars.
#[derive(Clone)]
pub struct Lang {
    name: &'static str,
    inner: DynLang,
}

impl Lang {
    pub fn new(name: &'static str, inner: DynLang) -> Self {
        Self { name, inner }
    }

    /// Get the language name.
    pub fn name(&self) -> &str {
        self.name
    }

    /// All available languages from the normalize-languages registry.
    pub fn all_langs() -> Vec<Self> {
        let loader = grammar_loader();
        normalize_languages::supported_languages()
            .into_iter()
            .filter_map(|lang| {
                let grammar_name = lang.grammar_name();
                let ts_lang = loader.get(grammar_name).ok()?;
                Some(Lang::new(intern_name(grammar_name), DynLang::new(ts_lang)))
            })
            .collect()
    }

    /// Resolve a language from a file path using normalize-languages.
    pub fn from_path_impl(path: &Path) -> Option<Self> {
        let lang_support = normalize_languages::support_for_path(path)?;
        let grammar_name = lang_support.grammar_name();
        let loader = grammar_loader();
        let ts_lang = loader.get(grammar_name).ok()?;
        Some(Lang::new(intern_name(grammar_name), DynLang::new(ts_lang)))
    }

    /// Build file type filters for this language using ignore crate.
    pub fn file_types(&self) -> Types {
        lang_globs::file_types_for_lang(self)
    }

    /// Injectable sub-languages (e.g., JS inside HTML).
    /// Not yet implemented — returns None.
    pub fn injectable_sg_langs(&self) -> Option<impl Iterator<Item = Self>> {
        None::<std::iter::Empty<Self>>
    }

    /// Augmented file types including injectors.
    pub fn augmented_file_type(&self) -> Types {
        self.file_types()
    }

    /// Merge file types for multiple languages.
    pub fn file_types_for_langs(langs: impl Iterator<Item = Self>) -> Types {
        lang_globs::merge_types(langs.map(|lang| lang.file_types()))
    }
}

fn grammar_loader() -> &'static GrammarLoader {
    static LOADER: OnceLock<GrammarLoader> = OnceLock::new();
    LOADER.get_or_init(GrammarLoader::new)
}

// Intern language name strings to get 'static lifetimes.
// This is safe because grammar names are loaded once and never freed.
static INTERNED: OnceLock<std::sync::Mutex<Vec<&'static str>>> = OnceLock::new();

fn intern_name(name: &str) -> &'static str {
    let mutex = INTERNED.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    let mut interned = mutex.lock().unwrap();
    // Check if already interned
    for &existing in interned.iter() {
        if existing == name {
            return existing;
        }
    }
    // Leak the string to get 'static lifetime
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    interned.push(leaked);
    leaked
}

impl PartialEq for Lang {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Lang {}

impl Hash for Lang {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Display for Lang {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Debug for Lang {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.name)
    }
}

#[derive(Debug)]
pub struct LangErr {
    pub name: String,
}

impl Display for LangErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is not supported!", self.name)
    }
}

impl std::error::Error for LangErr {}

impl FromStr for Lang {
    type Err = LangErr;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let name_lower = s.to_lowercase();
        // Map common aliases to grammar names
        let grammar_name = match name_lower.as_str() {
            "js" | "javascript" => "javascript",
            "ts" | "typescript" => "typescript",
            "tsx" => "tsx",
            "jsx" => "javascript",
            "py" | "python" => "python",
            "rs" | "rust" => "rust",
            "go" | "golang" => "go",
            "rb" | "ruby" => "ruby",
            "java" => "java",
            "kt" | "kotlin" => "kotlin",
            "swift" => "swift",
            "c" => "c",
            "cpp" | "cc" | "cxx" | "c++" => "cpp",
            "cs" | "csharp" | "c#" => "c-sharp",
            "php" => "php",
            "sh" | "bash" | "shell" => "bash",
            "lua" => "lua",
            "r" => "r",
            "scala" => "scala",
            "hs" | "haskell" => "haskell",
            "ml" | "ocaml" => "ocaml",
            "ex" | "elixir" => "elixir",
            "erl" | "erlang" => "erlang",
            "clj" | "clojure" => "clojure",
            "dart" => "dart",
            "html" => "html",
            "css" => "css",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "sql" => "sql",
            "zig" => "zig",
            "nim" => "nim",
            "julia" | "jl" => "julia",
            "nix" => "nix",
            other => other,
        };

        let loader = grammar_loader();
        let ts_lang = loader.get(grammar_name).map_err(|_| LangErr {
            name: s.to_string(),
        })?;
        Ok(Lang::new(intern_name(grammar_name), DynLang::new(ts_lang)))
    }
}

impl Serialize for Lang {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name)
    }
}

impl<'de> Deserialize<'de> for Lang {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Lang::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl AstGrepLanguage for Lang {
    fn kind_to_id(&self, kind: &str) -> u16 {
        self.inner.kind_to_id(kind)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.inner.field_to_id(field)
    }

    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        Self::from_path_impl(path.as_ref())
    }

    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        let lang = self.clone();
        builder.build(|src| StrDoc::try_new(src, lang))
    }
}

impl LanguageExt for Lang {
    fn get_ts_language(&self) -> TSLanguage {
        self.inner.get_ts_language()
    }

    // No injection support yet — returns None
    fn injectable_languages(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn extract_injections<L: LanguageExt>(
        &self,
        _root: Node<StrDoc<L>>,
    ) -> HashMap<String, Vec<TSRange>> {
        HashMap::new()
    }
}
