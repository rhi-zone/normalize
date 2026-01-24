//! Package index ingestion for cross-platform package mapping.
//!
//! This module provides fetchers that pull metadata from package manager indices
//! (apt Sources, brew API, crates.io, etc.) to extract package information.
//!
//! Unlike the `ecosystem` feature which is project-focused (dependencies of a project),
//! this is registry-focused (what packages exist and what's their metadata).

mod types;

#[cfg(test)]
mod tests;

// Distro package managers
pub mod apk;
pub mod apt;
mod arch_common;
pub mod artix;
pub mod cachyos;
pub mod chaotic_aur;
pub mod copr;
pub mod dnf;
pub mod endeavouros;
pub mod freebsd;
pub mod gentoo;
pub mod guix;
pub mod manjaro;
pub mod nix;
pub mod opensuse;
pub mod pacman;
pub mod slackware;
pub mod ubuntu;
pub mod void;

// Windows package managers
pub mod choco;
pub mod msys2;
pub mod scoop;
pub mod winget;

// macOS
pub mod brew;
pub mod homebrew_casks;
pub mod macports;

// Cross-platform app stores
pub mod flatpak;
pub mod snap;

// Containers
pub mod docker;

// Mobile
pub mod fdroid;
pub mod termux;

// Language package managers
pub mod bioconductor;
pub mod cargo;
pub mod clojars;
pub mod composer;
pub mod conan;
pub mod conda;
pub mod cran;
pub mod ctan;
pub mod deno;
pub mod dub;
pub mod gem;
pub mod go;
pub mod hackage;
pub mod hex;
pub mod hunter;
pub mod jsr;
pub mod julia;
pub mod luarocks;
pub mod maven;
pub mod metacpan;
pub mod nimble;
pub mod npm;
pub mod nuget;
pub mod opam;
pub mod pip;
pub mod pub_dev;
pub mod racket;
pub mod vcpkg;

pub use types::{IndexError, PackageIndex, PackageIter, PackageMeta, VersionMeta};

use std::sync::OnceLock;

static INDEX_REGISTRY: OnceLock<Vec<&'static dyn PackageIndex>> = OnceLock::new();
static OPENSUSE_INDEX: OnceLock<opensuse::OpenSuse> = OnceLock::new();
static PACMAN_INDEX: OnceLock<pacman::Pacman> = OnceLock::new();
static ARTIX_INDEX: OnceLock<artix::Artix> = OnceLock::new();
static APK_INDEX: OnceLock<apk::Apk> = OnceLock::new();
static FREEBSD_INDEX: OnceLock<freebsd::FreeBsd> = OnceLock::new();
static VOID_INDEX: OnceLock<void::Void> = OnceLock::new();
static MANJARO_INDEX: OnceLock<manjaro::Manjaro> = OnceLock::new();
static APT_INDEX: OnceLock<apt::Apt> = OnceLock::new();
static DNF_INDEX: OnceLock<dnf::Dnf> = OnceLock::new();
static UBUNTU_INDEX: OnceLock<ubuntu::Ubuntu> = OnceLock::new();
static NIX_INDEX: OnceLock<nix::Nix> = OnceLock::new();
static CACHYOS_INDEX: OnceLock<cachyos::CachyOs> = OnceLock::new();
static ENDEAVOUROS_INDEX: OnceLock<endeavouros::EndeavourOs> = OnceLock::new();
static GENTOO_INDEX: OnceLock<gentoo::Gentoo> = OnceLock::new();
static GUIX_INDEX: OnceLock<guix::Guix> = OnceLock::new();
static SLACKWARE_INDEX: OnceLock<slackware::Slackware> = OnceLock::new();
static SCOOP_INDEX: OnceLock<scoop::Scoop> = OnceLock::new();
static CHOCO_INDEX: OnceLock<choco::Choco> = OnceLock::new();
static WINGET_INDEX: OnceLock<winget::Winget> = OnceLock::new();
static FLATPAK_INDEX: OnceLock<flatpak::Flatpak> = OnceLock::new();
static SNAP_INDEX: OnceLock<snap::Snap> = OnceLock::new();
static CONDA_INDEX: OnceLock<conda::Conda> = OnceLock::new();
static MAVEN_INDEX: OnceLock<maven::Maven> = OnceLock::new();
static DOCKER_INDEX: OnceLock<docker::Docker> = OnceLock::new();
static FDROID_INDEX: OnceLock<fdroid::FDroid> = OnceLock::new();
static MSYS2_INDEX: OnceLock<msys2::Msys2> = OnceLock::new();

fn init_builtin() -> Vec<&'static dyn PackageIndex> {
    vec![
        // Distro
        APK_INDEX.get_or_init(apk::Apk::all),
        APT_INDEX.get_or_init(apt::Apt::all),
        ARTIX_INDEX.get_or_init(artix::Artix::all),
        CACHYOS_INDEX.get_or_init(cachyos::CachyOs::all),
        &chaotic_aur::ChaoticAur,
        &copr::Copr,
        DNF_INDEX.get_or_init(dnf::Dnf::all),
        ENDEAVOUROS_INDEX.get_or_init(endeavouros::EndeavourOs::all),
        FREEBSD_INDEX.get_or_init(freebsd::FreeBsd::all),
        GENTOO_INDEX.get_or_init(gentoo::Gentoo::all),
        GUIX_INDEX.get_or_init(guix::Guix::all),
        MANJARO_INDEX.get_or_init(manjaro::Manjaro::all),
        NIX_INDEX.get_or_init(nix::Nix::all),
        OPENSUSE_INDEX.get_or_init(opensuse::OpenSuse::all),
        PACMAN_INDEX.get_or_init(pacman::Pacman::all),
        SLACKWARE_INDEX.get_or_init(slackware::Slackware::all),
        UBUNTU_INDEX.get_or_init(ubuntu::Ubuntu::all),
        VOID_INDEX.get_or_init(void::Void::all),
        // Windows
        CHOCO_INDEX.get_or_init(choco::Choco::all),
        MSYS2_INDEX.get_or_init(msys2::Msys2::all),
        SCOOP_INDEX.get_or_init(scoop::Scoop::all),
        WINGET_INDEX.get_or_init(winget::Winget::all),
        // macOS
        &brew::Brew,
        &homebrew_casks::HomebrewCasks,
        &macports::MacPorts,
        // Cross-platform
        FLATPAK_INDEX.get_or_init(flatpak::Flatpak::all),
        SNAP_INDEX.get_or_init(snap::Snap::all),
        // Containers
        DOCKER_INDEX.get_or_init(docker::Docker::all),
        // Mobile
        FDROID_INDEX.get_or_init(fdroid::FDroid::all),
        &termux::Termux,
        // Language
        &bioconductor::Bioconductor,
        &vcpkg::Vcpkg,
        &clojars::Clojars,
        &cargo::CargoIndex,
        &ctan::Ctan,
        &composer::Composer,
        &conan::Conan,
        CONDA_INDEX.get_or_init(conda::Conda::all),
        &cran::Cran,
        &deno::Deno,
        &dub::Dub,
        &gem::Gem,
        &go::Go,
        &hackage::Hackage,
        &hex::Hex,
        &hunter::Hunter,
        &jsr::Jsr,
        &julia::Julia,
        &luarocks::LuaRocks,
        MAVEN_INDEX.get_or_init(maven::Maven::all),
        &metacpan::MetaCpan,
        &nimble::Nimble,
        &npm::NpmIndex,
        &nuget::Nuget,
        &opam::Opam,
        &pip::PipIndex,
        &pub_dev::Pub,
        &racket::Racket,
    ]
}

/// Get a package index by ecosystem name.
pub fn get_index(name: &str) -> Option<&'static dyn PackageIndex> {
    let registry = INDEX_REGISTRY.get_or_init(init_builtin);
    registry.iter().find(|idx| idx.ecosystem() == name).copied()
}

/// List all available package index ecosystem names.
pub fn list_indices() -> Vec<&'static str> {
    let registry = INDEX_REGISTRY.get_or_init(init_builtin);
    registry.iter().map(|idx| idx.ecosystem()).collect()
}

/// Get all registered package indices.
pub fn all_indices() -> Vec<&'static dyn PackageIndex> {
    INDEX_REGISTRY.get_or_init(init_builtin).clone()
}
