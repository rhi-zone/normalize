//! Package index ingestion from distro and language registries.
//!
//! Provides the [`PackageIndex`] trait for fetching package metadata from
//! package manager indices (apt, brew, crates.io, npm, etc.).
//!
//! # Example
//!
//! ```ignore
//! use normalize_package_index::{get_index, PackageMeta};
//!
//! if let Some(brew) = get_index("brew") {
//!     if let Ok(pkg) = brew.fetch("ripgrep") {
//!         println!("{}: {} - {:?}", pkg.name, pkg.version, pkg.repository);
//!     }
//! }
//! ```

pub mod cache;
pub mod index;

pub use index::{
    IndexError, PackageIndex, PackageIter, PackageMeta, VersionMeta, all_indices, get_index,
    list_indices,
};
