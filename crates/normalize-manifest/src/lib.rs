//! Manifest file parsing for programming language ecosystems.
//!
//! Provides a uniform `ParsedManifest` type and parsers for:
//! - `Cargo.toml` (Rust/Cargo)
//! - `go.mod` (Go modules)
//! - `package.json` (npm/Node.js)
//! - `requirements.txt` (pip)
//! - `pyproject.toml` (PEP 621 / Poetry)

pub mod cargo;
pub mod go_mod;
pub mod npm;
pub mod pip;
pub mod pyproject;

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
    /// Ecosystem identifier: `"cargo"`, `"go"`, `"npm"`, `"pip"`, `"python"`.
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
    fn filename(&self) -> &'static str;

    /// Parse manifest content and return structured data.
    fn parse(&self, content: &str) -> Result<ParsedManifest, ManifestError>;
}

// ============================================================================
// Top-level convenience functions
// ============================================================================

/// Parse a manifest file by filename, dispatching to the correct parser.
///
/// Returns `None` if the filename is not recognized.
pub fn parse_manifest(filename: &str, content: &str) -> Option<ParsedManifest> {
    match filename {
        "Cargo.toml" => cargo::CargoParser.parse(content).ok(),
        "go.mod" => go_mod::GoModParser.parse(content).ok(),
        "package.json" => npm::NpmParser.parse(content).ok(),
        "requirements.txt" => pip::PipParser.parse(content).ok(),
        "pyproject.toml" => pyproject::PyprojectParser.parse(content).ok(),
        _ => None,
    }
}

/// Parse go.mod content to extract module information.
///
/// Convenience wrapper for `normalize-local-deps` internal use.
pub fn go_module(content: &str) -> Option<GoModule> {
    go_mod::parse_go_module(content)
}
