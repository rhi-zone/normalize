# normalize-manifest

Manifest file parsing for programming language ecosystems.

Provides `ParsedManifest`, `DeclaredDep`, and `DepKind` as a uniform output type, plus the `ManifestParser` trait implemented by 50+ ecosystem-specific parsers. Top-level dispatch via `parse_manifest(filename, content)` (exact filenames) and `parse_manifest_by_extension(ext, content)` (wildcard formats such as `.nimble`, `.cabal`, `.csproj`, `.rockspec`). Convenience helpers `go_module()` and `npm_entry_point()` are re-exported for use by `normalize-local-deps`. An optional `eval` feature adds runtime-backed parsing for Swift, Go, Ruby, and Elixir manifests.
