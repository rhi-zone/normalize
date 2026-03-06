# xtask

Cargo xtask crate for build automation tasks not expressible as standard `cargo` commands. Currently implements `cargo xtask build-grammars`, which compiles tree-sitter grammar crates from the Cargo registry into shared libraries (`.so`/`.dylib`) and copies workspace `locals.scm` query files alongside them. Marked `publish = false` — this is a development tool only, not published to crates.io.
