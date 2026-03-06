# normalize-package-index

Package index ingestion from distro and language registries.

Defines the `PackageIndex` trait and `PackageMeta`/`VersionMeta`/`IndexError` types for fetching metadata from package manager indices (apt, pacman, brew, crates.io, npm, pip, etc.). Unlike `normalize-ecosystems` which is project-focused, this crate is registry-focused: it ingests what packages exist across 60+ indices and normalizes their metadata. Exports `get_index(name)`, `list_indices()`, and `all_indices()` for registry access. Supports streaming iteration (`iter_all`) for large indices and handles compressed archive formats (gzip, xz, zstd) for bulk ingestion.
