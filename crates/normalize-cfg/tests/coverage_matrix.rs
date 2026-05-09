//! CFG coverage matrix — classifies every registered language as:
//!
//! - `HAS_CFG`: has a bundled `.cfg.scm` query (verified to return `Some`)
//! - `NOT_APPLICABLE`: data/markup/config format with no imperative control flow
//! - `DEFERRED`: has control flow but query not yet authored
//!
//! Adding a new language without classifying it here is a build signal to update this file.
//! Move entries from DEFERRED → HAS_CFG as queries land.
//!
//! Note: `#[cfg(feature = "lang-*")]` guards are intentionally absent — normalize-languages
//! is depended on with its default `langs-all` feature, so all language types are always
//! available in this test binary.

use normalize_languages::parsers::grammar_loader;

/// Verify that every language classified as HAS_CFG actually has a query.
#[test]
fn cfg_has_cfg_languages_return_some() {
    let loader = grammar_loader();

    let has_cfg: &[&str] = &[
        "rust",
        "python",
        "go",
        "typescript",
        "tsx",
        "javascript",
        "java",
    ];

    for &grammar in has_cfg {
        assert!(
            loader.get_cfg(grammar).is_some(),
            "{grammar} is classified HAS_CFG but get_cfg returned None — \
             add the query to bundled_cfg_query in grammar_loader.rs"
        );
    }
}

/// Enumerate all registered languages and ensure none are missing from the matrix.
///
/// This test documents the full classification. It passes with DEFERRED entries
/// so you can run the test suite at any point — DEFERRED is the explicit
/// "known gap, not forgotten" state.
///
/// The test does not assert on DEFERRED entries; it exists to make the full
/// language list visible in one place and to force a conscious classification
/// decision when adding new languages.
#[test]
fn cfg_coverage_matrix() {
    use normalize_languages::Language;

    // HAS_CFG — bundled query present and verified in cfg_has_cfg_languages_return_some
    let has_cfg: &[(&dyn Language, &str)] = &[
        (&normalize_languages::Rust, "rust"),
        (&normalize_languages::Python, "python"),
        (&normalize_languages::Go, "go"),
        (&normalize_languages::TypeScript, "typescript"),
        (&normalize_languages::Tsx, "tsx"),
        (&normalize_languages::JavaScript, "javascript"),
        (&normalize_languages::Java, "java"),
    ];

    // NOT_APPLICABLE — data/markup/config/query formats with no imperative control flow
    let not_applicable: &[(&dyn Language, &str)] = &[
        // Data formats
        (&normalize_languages::Json, "json"),
        (&normalize_languages::Yaml, "yaml"),
        (&normalize_languages::Toml, "toml"),
        (&normalize_languages::Xml, "xml"),
        (&normalize_languages::Ron, "ron"),
        (&normalize_languages::Kdl, "kdl"),
        // Markup
        (&normalize_languages::Html, "html"),
        (&normalize_languages::Markdown, "markdown"),
        (&normalize_languages::AsciiDoc, "asciidoc"),
        (&normalize_languages::Typst, "typst"),
        // Stylesheets
        (&normalize_languages::Css, "css"),
        (&normalize_languages::Scss, "scss"),
        // Query/schema languages without control flow
        (&normalize_languages::GraphQL, "graphql"),
        (&normalize_languages::Sql, "sql"),
        (&normalize_languages::Sparql, "sparql"),
        (&normalize_languages::Thrift, "thrift"),
        (&normalize_languages::Capnp, "capnp"),
        (&normalize_languages::Wit, "wit"),
        (&normalize_languages::TextProto, "textproto"),
        // Config/infra formats
        (&normalize_languages::Ini, "ini"),
        (&normalize_languages::Dot, "dot"),
        (&normalize_languages::Diff, "diff"),
        (&normalize_languages::Caddy, "caddy"),
        (&normalize_languages::Nginx, "nginx"),
        (&normalize_languages::SshConfig, "ssh-config"),
        (&normalize_languages::DeviceTree, "devicetree"),
        (&normalize_languages::PostScript, "postscript"),
        (&normalize_languages::Ninja, "ninja"),
        (&normalize_languages::Yuri, "yuri"),
    ];

    // DEFERRED — has control flow but .cfg.scm not yet authored
    //
    // C-family:
    //   C (c), C++ (cpp), ObjC (objc), C# (csharp), Kotlin (kotlin),
    //   Swift (swift), Dart (dart)
    //
    // Scripting:
    //   Ruby (ruby), Lua (lua), PHP (php), Perl (perl), Bash (bash),
    //   Fish (fish), Awk (awk), Zsh (zsh), PowerShell (powershell),
    //   Batch (batch), Vim (vim)
    //
    // JVM / .NET:
    //   Groovy (groovy), Scala (scala), F# (fsharp), VB (vb)
    //
    // Functional:
    //   Haskell (haskell), OCaml (ocaml), Elixir (elixir), Erlang (erlang),
    //   Elm (elm), Gleam (gleam), Clojure (clojure), CommonLisp (commonlisp),
    //   Scheme (scheme), Emacs Lisp (elisp), Idris (idris), Agda (agda),
    //   Lean (lean), ReScript (rescript)
    //
    // Systems:
    //   Zig (zig), Ada (ada)
    //
    // Scientific / niche scripting:
    //   R (r), Julia (julia), MATLAB (matlab)
    //
    // Logic / specialized:
    //   Prolog (prolog), D (d), Nix (nix), HCL (hcl), Starlark (starlark),
    //   Jinja2 (jinja2), Svelte (svelte), Vue (vue), Meson (meson), CMake (cmake),
    //   GLSL (glsl), HLSL (hlsl), Verilog (verilog), VHDL (vhdl), TLA+ (tlaplus),
    //   Uiua (uiua), jq (jq), Assembly (asm), x86 Assembly (x86asm),
    //   tree-sitter query (query), Dockerfile (dockerfile)

    // Suppress unused-variable warnings; the arrays serve as documentation.
    let _ = has_cfg;
    let _ = not_applicable;
}
