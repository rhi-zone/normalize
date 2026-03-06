# normalize-package-index/src/index

PackageIndex trait, core types, and all registry implementations.

`types.rs` defines `PackageIndex`, `PackageMeta`, `VersionMeta`, `PackageIter`, and `IndexError`. `mod.rs` maintains a static registry initialized by `init_builtin()` and exposes `get_index`, `list_indices`, and `all_indices`. Each remaining file implements `PackageIndex` for one registry: distro package managers (apt, pacman, apk, dnf, brew, nix, etc.), Windows managers (scoop, choco, winget), language registries (cargo, npm, pip, gem, hex, maven, hackage, luarocks, etc.), and platform stores (flatpak, snap, fdroid). Bulk-capable indices use compressed archive ingestion (gzip/xz/zstd via flate2/xz2/zstd) with parallel processing via rayon.
