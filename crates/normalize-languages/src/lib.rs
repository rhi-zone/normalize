//! Language support for moss.
//!
//! This crate provides the `Language` trait and implementations for
//! various programming languages. Each language struct IS its support implementation.
//!
//! Grammars are loaded dynamically from shared libraries via `GrammarLoader`.
//! Build grammars with `cargo xtask build-grammars`.
//!
//! # Feature Flags
//!
//! Languages are gated behind feature flags for customizability:
//! - `langs-all` (default): All languages
//! - `langs-core`: Common languages (Python, JS, TS, Rust, Go, Java, etc.)
//! - `langs-functional`: Haskell, OCaml, Elixir, etc.
//! - `langs-config`: JSON, YAML, TOML, HCL, etc.
//! - `lang-*`: Individual language flags
//!
//! # Example
//!
//! ```ignore
//! use normalize_languages::{Python, Language, support_for_path, GrammarLoader};
//! use std::path::Path;
//!
//! // Load grammars
//! let loader = GrammarLoader::new();
//! let python_grammar = loader.get("python").expect("grammar not found");
//!
//! // Static usage (compile-time known language):
//! println!("Python function kinds: {:?}", Python.function_kinds());
//!
//! // Dynamic lookup (from file path):
//! if let Some(support) = support_for_path(Path::new("foo.py")) {
//!     println!("Language: {}", support.name());
//! }
//! ```

pub mod ast_grep;
pub mod c_cpp;
mod component;
pub mod ecmascript;
pub mod external_packages;
pub mod ffi;
mod grammar_loader;
mod registry;
mod traits;

// Language implementations (feature-gated)
#[cfg(feature = "lang-ada")]
pub mod ada;
#[cfg(feature = "lang-agda")]
pub mod agda;
#[cfg(feature = "lang-asciidoc")]
pub mod asciidoc;
#[cfg(feature = "lang-asm")]
pub mod asm;
#[cfg(feature = "lang-awk")]
pub mod awk;
#[cfg(feature = "lang-bash")]
pub mod bash;
#[cfg(feature = "lang-batch")]
pub mod batch;
#[cfg(feature = "lang-c")]
pub mod c;
#[cfg(feature = "lang-caddy")]
pub mod caddy;
#[cfg(feature = "lang-capnp")]
pub mod capnp;
#[cfg(feature = "lang-clojure")]
pub mod clojure;
#[cfg(feature = "lang-cmake")]
pub mod cmake;
#[cfg(feature = "lang-commonlisp")]
pub mod commonlisp;
#[cfg(feature = "lang-cpp")]
pub mod cpp;
#[cfg(feature = "lang-csharp")]
pub mod csharp;
#[cfg(feature = "lang-css")]
pub mod css;
#[cfg(feature = "lang-d")]
pub mod d;
#[cfg(feature = "lang-dart")]
pub mod dart;
#[cfg(feature = "lang-devicetree")]
pub mod devicetree;
#[cfg(feature = "lang-diff")]
pub mod diff;
#[cfg(feature = "lang-dockerfile")]
pub mod dockerfile;
#[cfg(feature = "lang-dot")]
pub mod dot;
#[cfg(feature = "lang-elisp")]
pub mod elisp;
#[cfg(feature = "lang-elixir")]
pub mod elixir;
#[cfg(feature = "lang-elm")]
pub mod elm;
#[cfg(feature = "lang-erlang")]
pub mod erlang;
#[cfg(feature = "lang-fish")]
pub mod fish;
#[cfg(feature = "lang-fsharp")]
pub mod fsharp;
#[cfg(feature = "lang-gleam")]
pub mod gleam;
#[cfg(feature = "lang-glsl")]
pub mod glsl;
#[cfg(feature = "lang-go")]
pub mod go;
#[cfg(feature = "lang-graphql")]
pub mod graphql;
#[cfg(feature = "lang-groovy")]
pub mod groovy;
#[cfg(feature = "lang-haskell")]
pub mod haskell;
#[cfg(feature = "lang-hcl")]
pub mod hcl;
#[cfg(feature = "lang-hlsl")]
pub mod hlsl;
#[cfg(feature = "lang-html")]
pub mod html;
#[cfg(feature = "lang-idris")]
pub mod idris;
#[cfg(feature = "lang-ini")]
pub mod ini;
#[cfg(feature = "lang-java")]
pub mod java;
#[cfg(feature = "lang-javascript")]
pub mod javascript;
#[cfg(feature = "lang-jinja2")]
pub mod jinja2;
#[cfg(feature = "lang-jq")]
pub mod jq;
#[cfg(feature = "lang-json")]
pub mod json;
#[cfg(feature = "lang-julia")]
pub mod julia;
#[cfg(feature = "lang-kdl")]
pub mod kdl;
#[cfg(feature = "lang-kotlin")]
pub mod kotlin;
#[cfg(feature = "lang-lean")]
pub mod lean;
#[cfg(feature = "lang-lua")]
pub mod lua;
#[cfg(feature = "lang-markdown")]
pub mod markdown;
#[cfg(feature = "lang-matlab")]
pub mod matlab;
#[cfg(feature = "lang-meson")]
pub mod meson;
#[cfg(feature = "lang-nginx")]
pub mod nginx;
#[cfg(feature = "lang-ninja")]
pub mod ninja;
#[cfg(feature = "lang-nix")]
pub mod nix;
#[cfg(feature = "lang-objc")]
pub mod objc;
#[cfg(feature = "lang-ocaml")]
pub mod ocaml;
#[cfg(feature = "lang-perl")]
pub mod perl;
#[cfg(feature = "lang-php")]
pub mod php;
#[cfg(feature = "lang-postscript")]
pub mod postscript;
#[cfg(feature = "lang-powershell")]
pub mod powershell;
#[cfg(feature = "lang-prolog")]
pub mod prolog;
#[cfg(feature = "lang-python")]
pub mod python;
#[cfg(feature = "lang-query")]
pub mod query;
#[cfg(feature = "lang-r")]
pub mod r;
#[cfg(feature = "lang-rescript")]
pub mod rescript;
#[cfg(feature = "lang-ron")]
pub mod ron;
#[cfg(feature = "lang-ruby")]
pub mod ruby;
#[cfg(feature = "lang-rust")]
pub mod rust;
#[cfg(feature = "lang-scala")]
pub mod scala;
#[cfg(feature = "lang-scheme")]
pub mod scheme;
#[cfg(feature = "lang-scss")]
pub mod scss;
#[cfg(feature = "lang-sparql")]
pub mod sparql;
#[cfg(feature = "lang-sql")]
pub mod sql;
#[cfg(feature = "lang-sshconfig")]
pub mod sshconfig;
#[cfg(feature = "lang-starlark")]
pub mod starlark;
#[cfg(feature = "lang-svelte")]
pub mod svelte;
#[cfg(feature = "lang-swift")]
pub mod swift;
#[cfg(feature = "lang-textproto")]
pub mod textproto;
#[cfg(feature = "lang-thrift")]
pub mod thrift;
#[cfg(feature = "lang-tlaplus")]
pub mod tlaplus;
#[cfg(feature = "lang-toml")]
pub mod toml;
#[cfg(feature = "lang-typescript")]
pub mod typescript;
#[cfg(feature = "lang-typst")]
pub mod typst;
#[cfg(feature = "lang-uiua")]
pub mod uiua;
#[cfg(feature = "lang-vb")]
pub mod vb;
#[cfg(feature = "lang-verilog")]
pub mod verilog;
#[cfg(feature = "lang-vhdl")]
pub mod vhdl;
#[cfg(feature = "lang-vim")]
pub mod vim;
#[cfg(feature = "lang-vue")]
pub mod vue;
#[cfg(feature = "lang-wit")]
pub mod wit;
#[cfg(feature = "lang-x86asm")]
pub mod x86asm;
#[cfg(feature = "lang-xml")]
pub mod xml;
#[cfg(feature = "lang-yaml")]
pub mod yaml;
#[cfg(feature = "lang-yuri")]
pub mod yuri;
#[cfg(feature = "lang-zig")]
pub mod zig;
#[cfg(feature = "lang-zsh")]
pub mod zsh;

// Re-exports (always available)
pub use grammar_loader::GrammarLoader;
pub use registry::{
    register, support_for_extension, support_for_grammar, support_for_path, supported_languages,
    validate_unused_kinds_audit,
};
pub use traits::{
    EmbeddedBlock, Export, Import, Language, PackageSource, PackageSourceKind, Symbol, SymbolKind,
    Visibility, VisibilityMechanism, has_extension, simple_function_symbol, simple_symbol,
    skip_dotfiles,
};

// Re-export language structs (feature-gated)
#[cfg(feature = "lang-ada")]
pub use ada::Ada;
#[cfg(feature = "lang-agda")]
pub use agda::Agda;
#[cfg(feature = "lang-asciidoc")]
pub use asciidoc::AsciiDoc;
#[cfg(feature = "lang-asm")]
pub use asm::Asm;
#[cfg(feature = "lang-awk")]
pub use awk::Awk;
#[cfg(feature = "lang-bash")]
pub use bash::Bash;
#[cfg(feature = "lang-batch")]
pub use batch::Batch;
#[cfg(feature = "lang-c")]
pub use c::C;
#[cfg(feature = "lang-caddy")]
pub use caddy::Caddy;
#[cfg(feature = "lang-capnp")]
pub use capnp::Capnp;
#[cfg(feature = "lang-clojure")]
pub use clojure::Clojure;
#[cfg(feature = "lang-cmake")]
pub use cmake::CMake;
#[cfg(feature = "lang-commonlisp")]
pub use commonlisp::CommonLisp;
#[cfg(feature = "lang-cpp")]
pub use cpp::Cpp;
#[cfg(feature = "lang-csharp")]
pub use csharp::CSharp;
#[cfg(feature = "lang-css")]
pub use css::Css;
#[cfg(feature = "lang-d")]
pub use d::D;
#[cfg(feature = "lang-dart")]
pub use dart::Dart;
#[cfg(feature = "lang-devicetree")]
pub use devicetree::DeviceTree;
#[cfg(feature = "lang-diff")]
pub use diff::Diff;
#[cfg(feature = "lang-dockerfile")]
pub use dockerfile::Dockerfile;
#[cfg(feature = "lang-dot")]
pub use dot::Dot;
#[cfg(feature = "lang-elisp")]
pub use elisp::Elisp;
#[cfg(feature = "lang-elixir")]
pub use elixir::Elixir;
#[cfg(feature = "lang-elm")]
pub use elm::Elm;
#[cfg(feature = "lang-erlang")]
pub use erlang::Erlang;
#[cfg(feature = "lang-fish")]
pub use fish::Fish;
#[cfg(feature = "lang-fsharp")]
pub use fsharp::FSharp;
#[cfg(feature = "lang-gleam")]
pub use gleam::Gleam;
#[cfg(feature = "lang-glsl")]
pub use glsl::Glsl;
#[cfg(feature = "lang-go")]
pub use go::Go;
#[cfg(feature = "lang-graphql")]
pub use graphql::GraphQL;
#[cfg(feature = "lang-groovy")]
pub use groovy::Groovy;
#[cfg(feature = "lang-haskell")]
pub use haskell::Haskell;
#[cfg(feature = "lang-hcl")]
pub use hcl::Hcl;
#[cfg(feature = "lang-hlsl")]
pub use hlsl::Hlsl;
#[cfg(feature = "lang-html")]
pub use html::Html;
#[cfg(feature = "lang-idris")]
pub use idris::Idris;
#[cfg(feature = "lang-ini")]
pub use ini::Ini;
#[cfg(feature = "lang-java")]
pub use java::Java;
#[cfg(feature = "lang-javascript")]
pub use javascript::JavaScript;
#[cfg(feature = "lang-jinja2")]
pub use jinja2::Jinja2;
#[cfg(feature = "lang-jq")]
pub use jq::Jq;
#[cfg(feature = "lang-json")]
pub use json::Json;
#[cfg(feature = "lang-julia")]
pub use julia::Julia;
#[cfg(feature = "lang-kdl")]
pub use kdl::Kdl;
#[cfg(feature = "lang-kotlin")]
pub use kotlin::Kotlin;
#[cfg(feature = "lang-lean")]
pub use lean::Lean;
#[cfg(feature = "lang-lua")]
pub use lua::Lua;
#[cfg(feature = "lang-markdown")]
pub use markdown::Markdown;
#[cfg(feature = "lang-matlab")]
pub use matlab::Matlab;
#[cfg(feature = "lang-meson")]
pub use meson::Meson;
#[cfg(feature = "lang-nginx")]
pub use nginx::Nginx;
#[cfg(feature = "lang-ninja")]
pub use ninja::Ninja;
#[cfg(feature = "lang-nix")]
pub use nix::Nix;
#[cfg(feature = "lang-objc")]
pub use objc::ObjC;
#[cfg(feature = "lang-ocaml")]
pub use ocaml::OCaml;
#[cfg(feature = "lang-perl")]
pub use perl::Perl;
#[cfg(feature = "lang-php")]
pub use php::Php;
#[cfg(feature = "lang-postscript")]
pub use postscript::PostScript;
#[cfg(feature = "lang-powershell")]
pub use powershell::PowerShell;
#[cfg(feature = "lang-prolog")]
pub use prolog::Prolog;
#[cfg(feature = "lang-python")]
pub use python::Python;
#[cfg(feature = "lang-query")]
pub use query::Query;
#[cfg(feature = "lang-r")]
pub use r::R;
#[cfg(feature = "lang-rescript")]
pub use rescript::ReScript;
#[cfg(feature = "lang-ron")]
pub use ron::Ron;
#[cfg(feature = "lang-ruby")]
pub use ruby::Ruby;
#[cfg(feature = "lang-rust")]
pub use rust::Rust;
#[cfg(feature = "lang-scala")]
pub use scala::Scala;
#[cfg(feature = "lang-scheme")]
pub use scheme::Scheme;
#[cfg(feature = "lang-scss")]
pub use scss::Scss;
#[cfg(feature = "lang-sparql")]
pub use sparql::Sparql;
#[cfg(feature = "lang-sql")]
pub use sql::Sql;
#[cfg(feature = "lang-sshconfig")]
pub use sshconfig::SshConfig;
#[cfg(feature = "lang-starlark")]
pub use starlark::Starlark;
#[cfg(feature = "lang-svelte")]
pub use svelte::Svelte;
#[cfg(feature = "lang-swift")]
pub use swift::Swift;
#[cfg(feature = "lang-textproto")]
pub use textproto::TextProto;
#[cfg(feature = "lang-thrift")]
pub use thrift::Thrift;
#[cfg(feature = "lang-tlaplus")]
pub use tlaplus::TlaPlus;
#[cfg(feature = "lang-toml")]
pub use toml::Toml;
#[cfg(feature = "lang-typescript")]
pub use typescript::{Tsx, TypeScript};
#[cfg(feature = "lang-typst")]
pub use typst::Typst;
#[cfg(feature = "lang-uiua")]
pub use uiua::Uiua;
#[cfg(feature = "lang-vb")]
pub use vb::VB;
#[cfg(feature = "lang-verilog")]
pub use verilog::Verilog;
#[cfg(feature = "lang-vhdl")]
pub use vhdl::Vhdl;
#[cfg(feature = "lang-vim")]
pub use vim::Vim;
#[cfg(feature = "lang-vue")]
pub use vue::Vue;
#[cfg(feature = "lang-wit")]
pub use wit::Wit;
#[cfg(feature = "lang-x86asm")]
pub use x86asm::X86Asm;
#[cfg(feature = "lang-xml")]
pub use xml::Xml;
#[cfg(feature = "lang-yaml")]
pub use yaml::Yaml;
#[cfg(feature = "lang-yuri")]
pub use yuri::Yuri;
#[cfg(feature = "lang-zig")]
pub use zig::Zig;
#[cfg(feature = "lang-zsh")]
pub use zsh::Zsh;
