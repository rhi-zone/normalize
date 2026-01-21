//! Ecosystem implementations.
//!
//! # Extensibility
//!
//! Users can register custom ecosystems via [`register()`]:
//!
//! ```ignore
//! use rhizome_moss_packages::{Ecosystem, LockfileManager, register_ecosystem};
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

#[cfg(feature = "ecosystem-cargo")]
mod cargo;
#[cfg(feature = "ecosystem-composer")]
mod composer;
#[cfg(feature = "ecosystem-conan")]
mod conan;
#[cfg(feature = "ecosystem-deno")]
mod deno;
#[cfg(feature = "ecosystem-gem")]
mod gem;
#[cfg(feature = "ecosystem-go")]
mod go;
#[cfg(feature = "ecosystem-hex")]
mod hex;
#[cfg(feature = "ecosystem-maven")]
mod maven;
#[cfg(feature = "ecosystem-nix")]
mod nix;
#[cfg(feature = "ecosystem-npm")]
mod npm;
#[cfg(feature = "ecosystem-nuget")]
mod nuget;
#[cfg(feature = "ecosystem-python")]
mod python;

use crate::Ecosystem;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

#[cfg(feature = "ecosystem-cargo")]
pub use cargo::Cargo;
#[cfg(feature = "ecosystem-composer")]
pub use composer::Composer;
#[cfg(feature = "ecosystem-conan")]
pub use conan::Conan;
#[cfg(feature = "ecosystem-deno")]
pub use deno::Deno;
#[cfg(feature = "ecosystem-gem")]
pub use gem::Gem;
#[cfg(feature = "ecosystem-go")]
pub use go::Go;
#[cfg(feature = "ecosystem-hex")]
pub use hex::Hex;
#[cfg(feature = "ecosystem-maven")]
pub use maven::Maven;
#[cfg(feature = "ecosystem-nix")]
pub use nix::Nix;
#[cfg(feature = "ecosystem-npm")]
pub use npm::Npm;
#[cfg(feature = "ecosystem-nuget")]
pub use nuget::Nuget;
#[cfg(feature = "ecosystem-python")]
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
        #[cfg(feature = "ecosystem-cargo")]
        ecosystems.push(&Cargo);
        #[cfg(feature = "ecosystem-npm")]
        ecosystems.push(&Npm);
        #[cfg(feature = "ecosystem-deno")]
        ecosystems.push(&Deno);
        #[cfg(feature = "ecosystem-python")]
        ecosystems.push(&Python);
        #[cfg(feature = "ecosystem-go")]
        ecosystems.push(&Go);
        #[cfg(feature = "ecosystem-hex")]
        ecosystems.push(&Hex);
        #[cfg(feature = "ecosystem-gem")]
        ecosystems.push(&Gem);
        #[cfg(feature = "ecosystem-composer")]
        ecosystems.push(&Composer);
        #[cfg(feature = "ecosystem-maven")]
        ecosystems.push(&Maven);
        #[cfg(feature = "ecosystem-nuget")]
        ecosystems.push(&Nuget);
        #[cfg(feature = "ecosystem-nix")]
        ecosystems.push(&Nix);
        #[cfg(feature = "ecosystem-conan")]
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
