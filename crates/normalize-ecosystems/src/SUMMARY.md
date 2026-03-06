# normalize-ecosystems/src

Source for the normalize-ecosystems crate.

`lib.rs` defines the `Ecosystem` trait, all shared data types, and the default `query()` / `detect_tool()` / `find_tool()` implementations. `ecosystems/` contains one module per ecosystem. `cache.rs` provides the on-disk JSON cache keyed by ecosystem + package name. `http.rs` provides a thin ureq wrapper with gzip support used by the registry-fetching ecosystem impls.
