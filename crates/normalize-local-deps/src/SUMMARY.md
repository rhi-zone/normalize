# normalize-local-deps/src

Source files for local dependency discovery.

- `lib.rs` — `LocalDeps` trait definition with all default method implementations; `ResolvedPackage`, `LocalDepSource`, `LocalDepSourceKind`; helper functions `skip_dotfiles`, `has_extension`
- `registry.rs` — `LocalDepsRegistry` mapping language names to `LocalDeps` implementations
- Per-ecosystem modules: `python.rs`, `javascript.rs`, `typescript.rs`, `rust_lang.rs`, `go.rs`, `java.rs`, `kotlin.rs`, `scala.rs`, `c.rs`, `cpp.rs`, `c_cpp.rs` (shared C/C++ logic), `ecmascript.rs` (shared JS/TS logic)
