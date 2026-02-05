//! Registry for language metadata.
//!
//! Provides lookup by language name (as returned by `Language::name()`).

use crate::Capabilities;
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
        && let Some(caps) = lock.read().unwrap().get(language_name)
    {
        return *caps;
    }

    // Built-in lookup
    builtin_capabilities(language_name)
}

/// Register custom capabilities for a language.
///
/// This allows extending or overriding the built-in capabilities.
pub fn register(language_name: impl Into<String>, capabilities: Capabilities) {
    let lock = USER_CAPABILITIES.get_or_init(|| RwLock::new(HashMap::new()));
    lock.write()
        .unwrap()
        .insert(language_name.into(), capabilities);
}

/// Built-in capabilities lookup.
/// Names must match `Language::name()` exactly (case-sensitive).
fn builtin_capabilities(name: &str) -> Capabilities {
    match name {
        // === Data formats (no code semantics) ===
        "JSON" => Capabilities::data_format(),
        "YAML" => Capabilities::data_format(),
        "TOML" => Capabilities::data_format(),
        "XML" => Capabilities::data_format(),
        "INI" => Capabilities::data_format(),
        "RON" => Capabilities::data_format(), // Rusty Object Notation
        "KDL" => Capabilities::data_format(),
        "TextProto" => Capabilities::data_format(),

        // === Markup languages ===
        "Markdown" => Capabilities::markup(),
        "AsciiDoc" => Capabilities::markup(),
        "HTML" => Capabilities {
            imports: true, // <script src>, <link href>
            callable_symbols: true,
            complexity: false,
            executable: false,
        },
        "CSS" | "SCSS" => Capabilities {
            imports: true, // @import
            callable_symbols: false,
            complexity: false,
            executable: false,
        },

        // === Query languages ===
        "SQL" => Capabilities::query(),
        "GraphQL" => Capabilities {
            imports: false,
            callable_symbols: true, // queries, mutations, types
            complexity: false,
            executable: true,
        },
        "SPARQL" => Capabilities::query(),
        "jq" => Capabilities::query(),

        // === Build/config DSLs ===
        "Dockerfile" => Capabilities::build_dsl(),
        "CMake" => Capabilities::build_dsl(),
        "Meson" => Capabilities::build_dsl(),
        "Ninja" => Capabilities::build_dsl(),
        "Starlark" => Capabilities::build_dsl(), // Bazel
        "HCL" => Capabilities::build_dsl(),      // Terraform
        "Nix" => Capabilities::build_dsl(),
        "Caddy" => Capabilities::build_dsl(), // Caddyfile

        // === Config files (limited code semantics) ===
        "Nginx" => Capabilities {
            imports: true, // include directive
            callable_symbols: false,
            complexity: false,
            executable: false,
        },
        "SSH Config" => Capabilities::data_format(),

        // === Shell languages ===
        "Bash" => Capabilities::shell(),
        "Zsh" => Capabilities::shell(),
        "Fish" => Capabilities::shell(),
        "PowerShell" => Capabilities::shell(),
        "Batch" => Capabilities::shell(),
        "AWK" => Capabilities::shell(),

        // === Diff/patch (not really a language) ===
        "Diff" => Capabilities::none(),

        // === Schema/IDL languages ===
        "Cap'n Proto" => Capabilities {
            imports: true,
            callable_symbols: true, // types, methods
            complexity: false,
            executable: false,
        },
        "Thrift" => Capabilities {
            imports: true,
            callable_symbols: true,
            complexity: false,
            executable: false,
        },
        "WIT" => Capabilities {
            // WebAssembly Interface Types
            imports: true,
            callable_symbols: true,
            complexity: false,
            executable: false,
        },

        // === Shader languages ===
        "GLSL" | "HLSL" => Capabilities {
            imports: false, // #include is preprocessor
            callable_symbols: true,
            complexity: true,
            executable: true,
        },

        // === Assembly languages ===
        "Assembly" | "x86 Assembly" => Capabilities {
            imports: false,
            callable_symbols: true, // labels
            complexity: false,      // metrics don't apply well
            executable: true,
        },

        // === Device/hardware description ===
        "DeviceTree" => Capabilities::data_format(),
        "Verilog" | "VHDL" => Capabilities {
            imports: true,
            callable_symbols: true,
            complexity: true,
            executable: false, // describes hardware, not executed
        },

        // === Documentation/literate ===
        "Typst" => Capabilities::markup(),

        // === Domain-specific ===
        "PostScript" => Capabilities {
            imports: false,
            callable_symbols: true,
            complexity: true,
            executable: true,
        },
        "DOT" => Capabilities::data_format(), // GraphViz
        "TLA+" => Capabilities {
            imports: true,
            callable_symbols: true,
            complexity: false, // specification language
            executable: false,
        },

        // === Templating (mixed markup/code) ===
        "Jinja2" => Capabilities {
            imports: true,
            callable_symbols: false,
            complexity: false,
            executable: false,
        },
        "Vue" | "Svelte" => Capabilities::all(), // Full component languages

        // === Everything else is a full programming language ===
        _ => Capabilities::all(),
    }
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
