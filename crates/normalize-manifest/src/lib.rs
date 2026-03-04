//! Manifest file parsing for programming language ecosystems.
//!
//! Provides a uniform `ParsedManifest` type and parsers for common package
//! manifest formats.  See `docs/manifest-support.md` for full coverage status.
//!
//! ## Dispatch
//!
//! - `parse_manifest(filename, content)` — dispatches by exact filename
//! - `parse_manifest_by_extension(ext, content)` — for wildcard-named files
//!   (`.nimble`, `.cabal`, `.csproj`, `.rockspec`)
//!
//! ## Convenience helpers
//!
//! - `go_module(content)` — extract module info from `go.mod`
//! - `npm_entry_point(content)` — extract entry point from `package.json`

#[cfg(feature = "eval")]
pub mod eval;

pub mod cabal;
pub mod cargo;
pub mod composer;
pub mod conan;
pub mod dub;
pub mod flake;
pub mod gemfile;
pub mod go_mod;
pub mod gradle;
pub mod maven;
pub mod mix_exs;
pub mod nimble;
pub mod npm;
pub mod nuget;
pub mod pip;
pub mod pubspec;
pub mod pyproject;
pub mod rockspec;
pub mod sbt;
pub mod setup_cfg;
pub mod stack;
pub mod swift_pm;

pub use go_mod::GoModule;
pub use npm::npm_entry_point;

use serde::Serialize;

// ============================================================================
// Core types
// ============================================================================

/// The kind of dependency relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DepKind {
    Normal,
    Dev,
    Build,
    Optional,
}

/// A declared dependency extracted from a manifest.
#[derive(Debug, Clone, Serialize)]
pub struct DeclaredDep {
    pub name: String,
    /// Version requirement string (e.g., `"^1.0"`, `">=2"`, `"v0.9.1"`).
    pub version_req: Option<String>,
    pub kind: DepKind,
}

/// Parsed contents of a project manifest file.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedManifest {
    /// Ecosystem identifier: `"cargo"`, `"go"`, `"npm"`, `"pip"`, `"python"`,
    /// `"composer"`, `"maven"`, `"gradle"`, `"sbt"`, `"hex"`, `"pub"`,
    /// `"bundler"`, `"conan"`, `"dub"`, `"nimble"`, `"cabal"`, `"luarocks"`,
    /// `"stackage"`, `"spm"`, `"nix"`, `"nuget"`.
    pub ecosystem: &'static str,
    pub name: Option<String>,
    pub version: Option<String>,
    pub dependencies: Vec<DeclaredDep>,
}

// ============================================================================
// ManifestParser trait
// ============================================================================

/// Error returned by manifest parsers.
#[derive(Debug)]
pub struct ManifestError(pub String);

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A parser for a specific manifest file format.
pub trait ManifestParser: Send + Sync {
    /// The canonical filename this parser handles (e.g., `"Cargo.toml"`).
    /// Extension-based parsers use a glob pattern like `"*.nimble"`.
    fn filename(&self) -> &'static str;

    /// Parse manifest content and return structured data.
    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError>;
}

// ============================================================================
// Top-level convenience functions
// ============================================================================

/// Parse a manifest file by exact filename, dispatching to the correct parser.
///
/// Returns `None` if the filename is not recognized. For extension-based formats
/// (`.nimble`, `.cabal`, `.csproj`, `.rockspec`), use `parse_manifest_by_extension`.
pub fn parse_manifest(filename: &str, content: &str) -> Option<ParsedManifest> {
    match filename {
        // Rust
        "Cargo.toml" => cargo::CargoParser.parse(content).ok(),
        // Go
        "go.mod" => go_mod::GoModParser.parse(content).ok(),
        // Node / npm
        "package.json" => npm::NpmParser.parse(content).ok(),
        // Python
        "requirements.txt" => pip::PipParser.parse(content).ok(),
        "pyproject.toml" => pyproject::PyprojectParser.parse(content).ok(),
        "setup.cfg" => setup_cfg::SetupCfgParser.parse(content).ok(),
        // PHP
        "composer.json" => composer::ComposerParser.parse(content).ok(),
        // JVM
        "pom.xml" => maven::MavenParser.parse(content).ok(),
        "build.gradle" => gradle::GradleParser.parse(content).ok(),
        "build.gradle.kts" => gradle::GradleKtsParser.parse(content).ok(),
        "build.sbt" => sbt::SbtParser.parse(content).ok(),
        // Elixir
        "mix.exs" => mix_exs::MixExsParser.parse(content).ok(),
        // Ruby
        "Gemfile" => gemfile::GemfileParser.parse(content).ok(),
        // Dart/Flutter
        "pubspec.yaml" => pubspec::PubspecParser.parse(content).ok(),
        // C/C++ (Conan)
        "conanfile.txt" => conan::ConanTxtParser.parse(content).ok(),
        "conanfile.py" => conan::ConanPyParser.parse(content).ok(),
        // .NET/NuGet
        "packages.config" => nuget::PackagesConfigParser.parse(content).ok(),
        // D language
        "dub.json" => dub::DubJsonParser.parse(content).ok(),
        "dub.sdl" => dub::DubSdlParser.parse(content).ok(),
        // Haskell
        "stack.yaml" => stack::StackParser.parse(content).ok(),
        // Nix
        "flake.nix" => flake::FlakeParser.parse(content).ok(),
        // Swift
        "Package.swift" => swift_pm::SwiftPmParser.parse(content).ok(),
        // Extension-based dispatch (wildcard filenames)
        _ => parse_manifest_by_extension_impl(filename, content),
    }
}

/// Parse a manifest file whose format is identified by file extension.
///
/// Handles: `.nimble`, `.cabal`, `.csproj`, `.rockspec`.
///
/// `filename` is the full filename (e.g. `"mypkg.nimble"`) or just the
/// extension (e.g. `"nimble"`). Either form is accepted.
pub fn parse_manifest_by_extension(filename: &str, content: &str) -> Option<ParsedManifest> {
    parse_manifest_by_extension_impl(filename, content)
}

fn parse_manifest_by_extension_impl(filename: &str, content: &str) -> Option<ParsedManifest> {
    let ext = filename.rsplit('.').next().unwrap_or(filename);

    match ext {
        "nimble" => nimble::NimbleParser.parse(content).ok(),
        "cabal" => cabal::CabalParser.parse(content).ok(),
        "csproj" | "vbproj" | "fsproj" => nuget::CsprojParser.parse(content).ok(),
        "rockspec" => rockspec::RockspecParser.parse(content).ok(),
        _ => None,
    }
}

// ============================================================================
// Eval-backed parsing (feature = "eval")
// ============================================================================

/// Controls what happens when the runtime needed for eval is not available.
#[cfg(feature = "eval")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalPolicy {
    /// Try eval; silently fall back to the heuristic parser if the runtime is
    /// absent or the command fails.
    IfAvailable,
    /// Return `None` if eval fails instead of falling back to heuristics.
    Required,
}

/// Parse a manifest file, preferring runtime evaluation over heuristics.
///
/// Dispatches to an eval-backed parser when the language runtime is available
/// (`swift`, `go`, `ruby`/`bundle`, `elixir`/`mix`). On failure or when the
/// runtime is absent, falls back to `parse_manifest` unless `policy` is
/// `EvalPolicy::Required`.
///
/// # Supported eval targets
///
/// | File | Command |
/// |------|---------|
/// | `Package.swift` | `swift package dump-package` |
/// | `go.mod` | `go mod edit -json` |
/// | `Gemfile` | `bundle exec ruby -e '…'` |
/// | `mix.exs` | `elixir -e '…'` |
///
/// All other filenames fall through to `parse_manifest` immediately.
#[cfg(feature = "eval")]
pub fn parse_manifest_eval(
    filename: &str,
    content: &str,
    root: &std::path::Path,
    policy: EvalPolicy,
) -> Option<ParsedManifest> {
    match eval::try_eval(filename, root) {
        Some(m) => Some(m),
        None if policy == EvalPolicy::IfAvailable => parse_manifest(filename, content),
        None => None,
    }
}

/// Parse go.mod content to extract module information.
///
/// Convenience wrapper for `normalize-local-deps` internal use.
pub fn go_module(content: &str) -> Option<GoModule> {
    go_mod::parse_go_module(content)
}
