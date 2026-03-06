# normalize-package-index/src

Source for the normalize-package-index crate.

`lib.rs` re-exports the public API from `index/`. `cache.rs` provides on-disk caching for fetched package metadata. `index/` contains all trait definitions and per-registry implementations.
