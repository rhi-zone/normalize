# normalize-grammars

Marker crate that declares dependencies on all arborium tree-sitter grammar crates (`publish = false`).

Not published to crates.io. Its sole purpose is to populate `~/.cargo/registry` with arborium grammar sources via `cargo fetch -p normalize-grammars`, enabling `cargo xtask build-grammars` to find and compile them. Contains ~90 `arborium-*` wildcard dependencies covering all supported languages. The `lib.rs` is empty.
