//! Registry for language metadata.
//!
//! Provides lookup by language name (as returned by `Language::name()`).

use crate::Capabilities;
use crate::data::LanguageIndex;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// User-registered language capabilities (for extending built-ins).
static USER_CAPABILITIES: OnceLock<RwLock<HashMap<String, Capabilities>>> = OnceLock::new();

/// Get capabilities for a language by name.
///
/// Returns `Capabilities::all()` for unknown languages (safe default - assume full capabilities).
pub fn capabilities_for(language_name: &str) -> Capabilities {
    // Check user-registered first
    if let Some(lock) = USER_CAPABILITIES.get()
        // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
        && let Some(caps) = lock.read().unwrap().get(language_name)
    {
        return *caps;
    }

    // Built-in lookup via LanguageIndex; unknown languages default to all().
    LanguageIndex::get()
        .capabilities_for(language_name)
        .unwrap_or_else(Capabilities::all)
}

/// Register custom capabilities for a language.
///
/// This allows extending or overriding the built-in capabilities.
pub fn register(language_name: impl Into<String>, capabilities: Capabilities) {
    let lock = USER_CAPABILITIES.get_or_init(|| RwLock::new(HashMap::new()));
    // normalize-syntax-allow: rust/unwrap-in-impl - mutex poison on a global registry is unrecoverable
    lock.write()
        .unwrap()
        .insert(language_name.into(), capabilities);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_programming_languages_have_all_capabilities() {
        for lang in ["Rust", "Python", "JavaScript", "TypeScript", "Go", "Java"] {
            let caps = capabilities_for(lang);
            assert!(caps.imports, "{} should have imports", lang);
            assert!(
                caps.callable_symbols,
                "{} should have callable_symbols",
                lang
            );
            assert!(caps.complexity, "{} should have complexity", lang);
            assert!(caps.executable, "{} should be executable", lang);
        }
    }

    #[test]
    fn test_data_formats_have_no_capabilities() {
        // Note: names must match Language::name() exactly
        for lang in ["JSON", "YAML", "TOML", "XML"] {
            let caps = capabilities_for(lang);
            assert!(!caps.imports, "{} should not have imports", lang);
            assert!(!caps.executable, "{} should not be executable", lang);
        }
    }

    #[test]
    fn test_unknown_language_defaults_to_all() {
        let caps = capabilities_for("UnknownLanguage2099");
        assert_eq!(caps, Capabilities::all());
    }

    #[test]
    fn test_user_registration_overrides_builtin() {
        register("JSON", Capabilities::all());
        let caps = capabilities_for("JSON");
        assert!(caps.imports); // Overridden
    }

    #[test]
    fn test_markup_languages() {
        let caps = capabilities_for("Markdown");
        assert!(!caps.imports);
        assert!(caps.callable_symbols); // headings are symbols
        assert!(!caps.executable);
    }

    #[test]
    fn test_shell_languages() {
        for lang in ["Bash", "Zsh", "Fish", "PowerShell"] {
            let caps = capabilities_for(lang);
            assert!(caps.imports, "{} should have imports (source)", lang);
            assert!(caps.executable, "{} should be executable", lang);
        }
    }
}
