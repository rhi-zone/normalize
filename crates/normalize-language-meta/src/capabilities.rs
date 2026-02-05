//! Language capabilities - what analyses apply to each language.

/// Capabilities that determine which analyses apply to a language.
///
/// Most programming languages have all capabilities. Data formats like JSON
/// and markup languages like Markdown have reduced capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Language has an import/module system (use, import, require, include).
    /// False for: JSON, YAML, TOML, INI, XML (pure data formats).
    pub imports: bool,

    /// Language has callable symbols (functions, methods, procedures).
    /// False for: JSON, YAML, TOML, INI (pure data).
    /// True for: SQL (has functions), Markdown (has headings as "symbols").
    pub callable_symbols: bool,

    /// Complexity metrics (cyclomatic, cognitive) are meaningful.
    /// False for: data formats, markup, config files.
    pub complexity: bool,

    /// Language is primarily for executable code (vs data/config/docs).
    /// This is fuzzy - YAML can be "code" in GitHub Actions, but generally isn't.
    /// Use for filtering in architecture analysis, orphan detection, etc.
    pub executable: bool,
}

impl Capabilities {
    /// All capabilities enabled - typical for programming languages.
    pub const fn all() -> Self {
        Self {
            imports: true,
            callable_symbols: true,
            complexity: true,
            executable: true,
        }
    }

    /// No capabilities - for languages we haven't classified yet.
    pub const fn none() -> Self {
        Self {
            imports: false,
            callable_symbols: false,
            complexity: false,
            executable: false,
        }
    }

    /// Data format (JSON, YAML, TOML, etc.) - has structure but no code semantics.
    pub const fn data_format() -> Self {
        Self {
            imports: false,
            callable_symbols: false,
            complexity: false,
            executable: false,
        }
    }

    /// Markup language (Markdown, AsciiDoc, HTML) - has structure, may have symbols.
    pub const fn markup() -> Self {
        Self {
            imports: false,
            callable_symbols: true, // headings, sections are "symbols"
            complexity: false,
            executable: false,
        }
    }

    /// Query language (SQL, GraphQL) - has code semantics but different context.
    pub const fn query() -> Self {
        Self {
            imports: false, // typically no import system
            callable_symbols: true,
            complexity: true,
            executable: true,
        }
    }

    /// Build/config DSL (Dockerfile, Makefile, CMake) - imperative but constrained.
    pub const fn build_dsl() -> Self {
        Self {
            imports: true, // often have includes
            callable_symbols: true,
            complexity: true,
            executable: true,
        }
    }

    /// Shell/scripting language - full programming but often used for glue.
    pub const fn shell() -> Self {
        Self {
            imports: true, // source, ., include
            callable_symbols: true,
            complexity: true,
            executable: true,
        }
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Self::all()
    }
}
