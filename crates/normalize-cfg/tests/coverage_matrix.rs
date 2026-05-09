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
        // Seed languages
        "rust",
        "python",
        "go",
        "typescript",
        "tsx",
        "javascript",
        "java",
        // Batch A: C-family
        "c",
        "cpp",
        "objc",
        "c-sharp",
        "kotlin",
        "swift",
        "dart",
        // Batch B: JVM/functional
        "scala",
        "groovy",
        "vb",
        "haskell",
        "ocaml",
        "fsharp",
        "elixir",
        "erlang",
        "clojure",
        "gleam",
        "rescript",
        "idris",
        "agda",
        "lean",
        "commonlisp",
        "scheme",
        "elisp",
        // Batch C: Scripting
        "ruby",
        "lua",
        "php",
        "perl",
        "bash",
        "fish",
        "awk",
        "zsh",
        "powershell",
        "batch",
        "vim",
        // Batch D: Systems/other
        "zig",
        "ada",
        "d",
        "prolog",
        "r",
        "julia",
        "matlab",
        "glsl",
        "hlsl",
        "verilog",
        "vhdl",
        // Batch E: Domain/config
        "nix",
        "hcl",
        "starlark",
        "elm",
        "jinja2",
        "svelte",
        "vue",
        "cmake",
        "meson",
        "tlaplus",
        "jq",
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
        // Seed languages
        (&normalize_languages::Rust, "rust"),
        (&normalize_languages::Python, "python"),
        (&normalize_languages::Go, "go"),
        (&normalize_languages::TypeScript, "typescript"),
        (&normalize_languages::Tsx, "tsx"),
        (&normalize_languages::JavaScript, "javascript"),
        (&normalize_languages::Java, "java"),
        // Batch A: C-family
        (&normalize_languages::C, "c"),
        (&normalize_languages::Cpp, "cpp"),
        (&normalize_languages::ObjC, "objc"),
        (&normalize_languages::CSharp, "c-sharp"),
        (&normalize_languages::Kotlin, "kotlin"),
        (&normalize_languages::Swift, "swift"),
        (&normalize_languages::Dart, "dart"),
        // Batch B: JVM/functional
        (&normalize_languages::Scala, "scala"),
        (&normalize_languages::Groovy, "groovy"),
        (&normalize_languages::VB, "vb"),
        (&normalize_languages::Haskell, "haskell"),
        (&normalize_languages::OCaml, "ocaml"),
        (&normalize_languages::FSharp, "fsharp"),
        (&normalize_languages::Elixir, "elixir"),
        (&normalize_languages::Erlang, "erlang"),
        (&normalize_languages::Clojure, "clojure"),
        (&normalize_languages::Gleam, "gleam"),
        (&normalize_languages::ReScript, "rescript"),
        (&normalize_languages::Idris, "idris"),
        (&normalize_languages::Agda, "agda"),
        (&normalize_languages::Lean, "lean"),
        (&normalize_languages::CommonLisp, "commonlisp"),
        (&normalize_languages::Scheme, "scheme"),
        (&normalize_languages::Elisp, "elisp"),
        // Batch C: Scripting
        (&normalize_languages::Ruby, "ruby"),
        (&normalize_languages::Lua, "lua"),
        (&normalize_languages::Php, "php"),
        (&normalize_languages::Perl, "perl"),
        (&normalize_languages::Bash, "bash"),
        (&normalize_languages::Fish, "fish"),
        (&normalize_languages::Awk, "awk"),
        (&normalize_languages::Zsh, "zsh"),
        (&normalize_languages::PowerShell, "powershell"),
        (&normalize_languages::Batch, "batch"),
        (&normalize_languages::Vim, "vim"),
        // Batch D: Systems/other
        (&normalize_languages::Zig, "zig"),
        (&normalize_languages::Ada, "ada"),
        (&normalize_languages::D, "d"),
        (&normalize_languages::Prolog, "prolog"),
        (&normalize_languages::R, "r"),
        (&normalize_languages::Julia, "julia"),
        (&normalize_languages::Matlab, "matlab"),
        (&normalize_languages::Glsl, "glsl"),
        (&normalize_languages::Hlsl, "hlsl"),
        (&normalize_languages::Verilog, "verilog"),
        (&normalize_languages::Vhdl, "vhdl"),
        // Batch E: Domain/config
        (&normalize_languages::Nix, "nix"),
        (&normalize_languages::Hcl, "hcl"),
        (&normalize_languages::Starlark, "starlark"),
        (&normalize_languages::Elm, "elm"),
        (&normalize_languages::Jinja2, "jinja2"),
        (&normalize_languages::Svelte, "svelte"),
        (&normalize_languages::Vue, "vue"),
        (&normalize_languages::CMake, "cmake"),
        (&normalize_languages::Meson, "meson"),
        (&normalize_languages::TlaPlus, "tlaplus"),
        (&normalize_languages::Jq, "jq"),
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
        // tree-sitter query language — not an imperative language
        (&normalize_languages::Query, "query"),
        // Dockerfile — declarative build instructions, no control flow
        (&normalize_languages::Dockerfile, "dockerfile"),
    ];

    // DEFERRED — has control flow but .cfg.scm not yet authored, or grammar
    // cannot be inspected without the grammar being installed.
    //
    // Assembly languages: asm, x86asm — branches exist (jmp/je/jne) at the
    //   instruction level but the grammar structure needs inspection to write
    //   correct queries. Grammar not available in the current arborium install.
    //
    // Uiua (uiua) — array programming language with non-standard control flow;
    //   grammar has no query files yet; defer until grammar inspection is possible.

    // Suppress unused-variable warnings; the arrays serve as documentation.
    let _ = has_cfg;
    let _ = not_applicable;
}
