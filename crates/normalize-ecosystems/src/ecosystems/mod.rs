//! Ecosystem implementations.
//!
//! # Extensibility
//!
//! Users can register custom ecosystems via [`register()`]:
//!
//! ```ignore
//! use normalize_ecosystems::{Ecosystem, LockfileManager, register_ecosystem};
//! use std::path::Path;
//!
//! struct MyEcosystem;
//!
//! impl Ecosystem for MyEcosystem {
//!     fn name(&self) -> &'static str { "my-ecosystem" }
//!     fn manifest_files(&self) -> &'static [&'static str] { &["my-manifest.json"] }
//!     fn lockfiles(&self) -> &'static [LockfileManager] { &[] }
//!     fn tools(&self) -> &'static [&'static str] { &["my-tool"] }
//!     // ... implement other methods
//! }
//!
//! // Register before first use
//! register_ecosystem(&MyEcosystem);
//! ```

#[cfg(feature = "cargo")]
mod cargo;
#[cfg(feature = "composer")]
mod composer;
#[cfg(feature = "conan")]
mod conan;
#[cfg(feature = "deno")]
mod deno;
#[cfg(feature = "gem")]
mod gem;
#[cfg(feature = "go")]
mod go;
#[cfg(feature = "hex")]
mod hex;
#[cfg(feature = "maven")]
mod maven;
#[cfg(feature = "nix")]
mod nix;
#[cfg(feature = "npm")]
mod npm;
#[cfg(feature = "nuget")]
mod nuget;
#[cfg(feature = "python")]
mod python;

use crate::Ecosystem;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

#[cfg(feature = "cargo")]
pub use cargo::Cargo;
#[cfg(feature = "composer")]
pub use composer::Composer;
#[cfg(feature = "conan")]
pub use conan::Conan;
#[cfg(feature = "deno")]
pub use deno::Deno;
#[cfg(feature = "gem")]
pub use gem::Gem;
#[cfg(feature = "go")]
pub use go::Go;
#[cfg(feature = "hex")]
pub use hex::Hex;
#[cfg(feature = "maven")]
pub use maven::Maven;
#[cfg(feature = "nix")]
pub use nix::Nix;
#[cfg(feature = "npm")]
pub use npm::Npm;
#[cfg(feature = "nuget")]
pub use nuget::Nuget;
#[cfg(feature = "python")]
pub use python::Python;

/// Global registry of ecosystem plugins.
static ECOSYSTEMS: RwLock<Vec<&'static dyn Ecosystem>> = RwLock::new(Vec::new());
static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Register a custom ecosystem plugin.
///
/// Call this before any detection operations to add custom ecosystems.
/// Built-in ecosystems are registered automatically on first use.
pub fn register(ecosystem: &'static dyn Ecosystem) {
    ECOSYSTEMS.write().unwrap().push(ecosystem);
}

/// Initialize built-in ecosystems (called automatically on first use).
fn init_builtin() {
    INITIALIZED.get_or_init(|| {
        let mut ecosystems = ECOSYSTEMS.write().unwrap();
        #[cfg(feature = "cargo")]
        ecosystems.push(&Cargo);
        #[cfg(feature = "npm")]
        ecosystems.push(&Npm);
        #[cfg(feature = "deno")]
        ecosystems.push(&Deno);
        #[cfg(feature = "python")]
        ecosystems.push(&Python);
        #[cfg(feature = "go")]
        ecosystems.push(&Go);
        #[cfg(feature = "hex")]
        ecosystems.push(&Hex);
        #[cfg(feature = "gem")]
        ecosystems.push(&Gem);
        #[cfg(feature = "composer")]
        ecosystems.push(&Composer);
        #[cfg(feature = "maven")]
        ecosystems.push(&Maven);
        #[cfg(feature = "nuget")]
        ecosystems.push(&Nuget);
        #[cfg(feature = "nix")]
        ecosystems.push(&Nix);
        #[cfg(feature = "conan")]
        ecosystems.push(&Conan);
    });
}

/// Get an ecosystem by name from the global registry.
pub fn get_ecosystem(name: &str) -> Option<&'static dyn Ecosystem> {
    init_builtin();
    ECOSYSTEMS
        .read()
        .unwrap()
        .iter()
        .find(|e| e.name() == name)
        .copied()
}

/// List all available ecosystem names from the global registry.
pub fn list_ecosystems() -> Vec<&'static str> {
    init_builtin();
    ECOSYSTEMS
        .read()
        .unwrap()
        .iter()
        .map(|e| e.name())
        .collect()
}

/// Detect ecosystem from project files.
pub fn detect_ecosystem(project_root: &Path) -> Option<&'static dyn Ecosystem> {
    detect_all_ecosystems(project_root).into_iter().next()
}

/// Detect all ecosystems from project files.
pub fn detect_all_ecosystems(project_root: &Path) -> Vec<&'static dyn Ecosystem> {
    init_builtin();
    let ecosystems = ECOSYSTEMS.read().unwrap();

    let mut found = Vec::new();
    for ecosystem in ecosystems.iter() {
        for manifest in ecosystem.manifest_files() {
            let matches = if manifest.contains('*') {
                // Glob pattern - check if any matching file exists
                if let Some(pattern) = manifest.strip_prefix('*') {
                    std::fs::read_dir(project_root)
                        .ok()
                        .map(|entries| {
                            entries
                                .flatten()
                                .any(|entry| entry.file_name().to_string_lossy().ends_with(pattern))
                        })
                        .unwrap_or(false)
                } else {
                    false
                }
            } else {
                project_root.join(manifest).exists()
            };

            if matches {
                found.push(*ecosystem);
                break; // Don't add same ecosystem twice for different manifest files
            }
        }
    }
    found
}

/// Get all registered ecosystems.
pub fn all_ecosystems() -> Vec<&'static dyn Ecosystem> {
    init_builtin();
    ECOSYSTEMS.read().unwrap().clone()
}
