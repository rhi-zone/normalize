# normalize-manifest/src

Source for the normalize-manifest crate.

`lib.rs` defines the core types (`ParsedManifest`, `DeclaredDep`, `DepKind`, `ManifestParser`) and the top-level dispatch functions `parse_manifest` and `parse_manifest_by_extension`. Each remaining file is a self-contained parser module for one ecosystem (e.g., `cargo.rs`, `npm.rs`, `pip.rs`, `maven.rs`). `sexpr.rs` provides shared s-expression parsing utilities used by Clojure, Common Lisp, and Racket parsers. `eval.rs` (behind the `eval` feature flag) handles runtime-backed parsing for manifests that require language toolchains to evaluate.
