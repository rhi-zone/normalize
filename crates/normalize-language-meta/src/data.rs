//! Shared loader for `data/languages.toml`.
//!
//! Parses the TOML file once via `OnceLock` and provides a `LanguageIndex`
//! used by both `registry` and `test_globs`.

use crate::Capabilities;
use std::collections::HashMap;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// TOML deserialization types
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
struct CapabilitiesData {
    preset: Option<String>,
    imports: Option<bool>,
    callable_symbols: Option<bool>,
    complexity: Option<bool>,
    executable: Option<bool>,
}

#[derive(serde::Deserialize)]
struct LanguageEntry {
    names: Vec<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    test_globs: Vec<String>,
    #[serde(default)]
    capabilities: CapabilitiesData,
}

#[derive(serde::Deserialize)]
struct RawData {
    language: Vec<LanguageEntry>,
}

// ---------------------------------------------------------------------------
// LanguageIndex
// ---------------------------------------------------------------------------

/// Pre-built index over `data/languages.toml`.
pub struct LanguageIndex {
    /// Exact canonical name (case-sensitive) → capabilities.
    by_name: HashMap<String, Capabilities>,
    /// Lowercase identifier (name / alias / extension, dot-stripped) → test globs.
    by_id: HashMap<String, Vec<String>>,
}

impl LanguageIndex {
    /// Return the global singleton, parsing `languages.toml` on first call.
    pub fn get() -> &'static LanguageIndex {
        static INDEX: OnceLock<LanguageIndex> = OnceLock::new();
        INDEX.get_or_init(|| {
            let raw = include_str!("../data/languages.toml");
            let data: RawData = toml::from_str(raw).expect("languages.toml is malformed");
            LanguageIndex::build(data.language)
        })
    }

    fn build(entries: Vec<LanguageEntry>) -> Self {
        let mut by_name: HashMap<String, Capabilities> = HashMap::new();
        let mut by_id: HashMap<String, Vec<String>> = HashMap::new();

        for entry in entries {
            let caps = resolve_capabilities(&entry.capabilities);

            // Insert every name into by_name (case-sensitive) with resolved caps.
            for name in &entry.names {
                by_name.entry(name.clone()).or_insert(caps);
            }

            // Insert lowercased names + extensions into by_id for test_globs lookup.
            if !entry.test_globs.is_empty() {
                let globs = entry.test_globs;
                for name in &entry.names {
                    by_id
                        .entry(name.to_ascii_lowercase())
                        .or_insert_with(|| globs.clone());
                }
                for ext in &entry.extensions {
                    let key = ext.trim_start_matches('.').to_ascii_lowercase();
                    by_id.entry(key).or_insert_with(|| globs.clone());
                }
            }
        }

        LanguageIndex { by_name, by_id }
    }

    /// Look up capabilities by exact canonical name (as returned by `Language::name()`).
    ///
    /// Returns `None` for names not listed in `languages.toml`; callers should
    /// fall back to `Capabilities::all()`.
    pub fn capabilities_for(&self, name: &str) -> Option<Capabilities> {
        self.by_name.get(name).copied()
    }

    /// Return test-file glob patterns for a language identifier.
    ///
    /// Accepts language names (case-insensitive), file extensions with or without
    /// a leading dot, and common aliases.  Returns an empty `Vec` for unknown
    /// languages or languages without a dedicated test-file convention.
    pub fn test_globs_for(&self, identifier: &str) -> Vec<String> {
        let key = identifier.trim_start_matches('.').to_ascii_lowercase();
        self.by_id.get(&key).cloned().unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Public free function
// ---------------------------------------------------------------------------

/// Return glob patterns that identify dedicated test files for the named language.
///
/// Accepts any reasonable identifier:
/// - Language names (case-insensitive): `"Go"`, `"go"`, `"JavaScript"`
/// - File extensions with or without leading dot: `".go"`, `"rs"`, `".py"`
/// - Common aliases: `"golang"`, `"csharp"`, `"js"`, `"ts"`, `"py"`, `"rb"`
///
/// Returns an empty `Vec` for unknown languages or languages without a dedicated
/// test file naming convention (e.g. C, C++, Zig — which use inline tests).
pub fn test_file_globs_for_language(name: &str) -> Vec<String> {
    LanguageIndex::get().test_globs_for(name)
}

// ---------------------------------------------------------------------------
// CapabilitiesData → Capabilities resolution
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_name() {
        assert!(!test_file_globs_for_language("Go").is_empty());
        assert!(!test_file_globs_for_language("Rust").is_empty());
        assert!(!test_file_globs_for_language("JavaScript").is_empty());
        assert!(!test_file_globs_for_language("TypeScript").is_empty());
    }

    #[test]
    fn test_lowercase_name() {
        assert!(!test_file_globs_for_language("go").is_empty());
        assert!(!test_file_globs_for_language("rust").is_empty());
        assert!(!test_file_globs_for_language("javascript").is_empty());
    }

    #[test]
    fn test_extensions_without_dot() {
        assert!(!test_file_globs_for_language("rs").is_empty());
        assert!(!test_file_globs_for_language("py").is_empty());
        assert!(!test_file_globs_for_language("js").is_empty());
        assert!(!test_file_globs_for_language("ts").is_empty());
        assert!(!test_file_globs_for_language("rb").is_empty());
        assert!(!test_file_globs_for_language("kt").is_empty());
    }

    #[test]
    fn test_extensions_with_dot() {
        assert!(!test_file_globs_for_language(".go").is_empty());
        assert!(!test_file_globs_for_language(".rs").is_empty());
        assert!(!test_file_globs_for_language(".py").is_empty());
        assert!(!test_file_globs_for_language(".js").is_empty());
        assert!(!test_file_globs_for_language(".ts").is_empty());
    }

    #[test]
    fn test_aliases() {
        assert!(!test_file_globs_for_language("golang").is_empty());
        assert!(!test_file_globs_for_language("csharp").is_empty());
        assert!(!test_file_globs_for_language("fsharp").is_empty());
        assert!(!test_file_globs_for_language("python").is_empty());
    }

    #[test]
    fn test_unknown_returns_empty() {
        assert!(test_file_globs_for_language("unknown").is_empty());
        assert!(test_file_globs_for_language("").is_empty());
        assert!(test_file_globs_for_language("c").is_empty());
    }

    #[test]
    fn test_data_file_parses_cleanly() {
        let _ = LanguageIndex::get();
    }
}

fn resolve_capabilities(data: &CapabilitiesData) -> Capabilities {
    let mut caps = match data.preset.as_deref() {
        Some("all") | None => Capabilities::all(),
        Some("data_format") => Capabilities::data_format(),
        Some("markup") => Capabilities::markup(),
        Some("query") => Capabilities::query(),
        Some("build_dsl") => Capabilities::build_dsl(),
        Some("shell") => Capabilities::shell(),
        Some("none") => Capabilities::none(),
        Some(other) => panic!("Unknown capabilities preset '{other}' in languages.toml"),
    };

    // Apply individual bool overrides on top of the preset.
    if let Some(v) = data.imports {
        caps.imports = v;
    }
    if let Some(v) = data.callable_symbols {
        caps.callable_symbols = v;
    }
    if let Some(v) = data.complexity {
        caps.complexity = v;
    }
    if let Some(v) = data.executable {
        caps.executable = v;
    }

    caps
}
