# Manifest Support

Coverage status for the `normalize-manifest` crate.

Each row is a manifest file format. "Parser" = a `ManifestParser` impl exists in
`normalize-manifest`. "Extracts" = what that parser currently returns.

## Supported — Fixed Filename

Dispatched by `parse_manifest(filename, content)`.

| Manifest | Ecosystem | Parser | Extracts |
|---|---|---|---|
| `Cargo.toml` | Rust / Cargo | `cargo::CargoParser` | name, version, `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` |
| `go.mod` | Go | `go_mod::GoModParser` | module path (→ name), go version, `require` directives |
| `package.json` | npm / Node.js | `npm::NpmParser` | name, version, `dependencies`, `devDependencies`, `peerDependencies` |
| `requirements.txt` | pip | `pip::PipParser` | deps with version specifiers |
| `pyproject.toml` | Python (PEP 621 + Poetry) | `pyproject::PyprojectParser` | name, version, `[project.dependencies]`, Poetry deps |
| `setup.cfg` | Python / setuptools | `setup_cfg::SetupCfgParser` | name, version, `install_requires`, extras |
| `composer.json` | PHP / Composer | `composer::ComposerParser` | name, version, `require`, `require-dev` (platform reqs filtered) |
| `pom.xml` | Java / Maven | `maven::MavenParser` | name, version, `<dependencies>` with scope → DepKind mapping |
| `build.gradle` | Java / Gradle (Groovy) | `gradle::GradleParser` | `dependencies { ... }` block, config → DepKind |
| `build.gradle.kts` | Java / Gradle (Kotlin) | `gradle::GradleKtsParser` | same as above |
| `build.sbt` | Scala / sbt | `sbt::SbtParser` | `libraryDependencies`, `Seq(...)` blocks, `% Test` scope |
| `mix.exs` | Elixir / Hex | `mix_exs::MixExsParser` | app name, version, `deps/0` function; `only: :dev` → Dev |
| `Gemfile` | Ruby / Bundler | `gemfile::GemfileParser` | `gem` declarations, `group :development` → Dev |
| `pubspec.yaml` | Dart / Flutter | `pubspec::PubspecParser` | name, version, `dependencies`, `dev_dependencies`; `sdk:` filtered |
| `conanfile.txt` | C / C++ (Conan v1) | `conan::ConanTxtParser` | `[requires]` section, `pkg/version@user/channel` |
| `conanfile.py` | C / C++ (Conan v1/v2) | `conan::ConanPyParser` | `requires = [...]` list, `self.requires(...)` calls |
| `packages.config` | .NET / NuGet (legacy) | `nuget::PackagesConfigParser` | `<package id=... version=.../>`, `developmentDependency` → Dev |
| `dub.json` | D / Dub | `dub::DubJsonParser` | `dependencies`, `devDependencies`, `optionalDependencies` |
| `dub.sdl` | D / Dub | `dub::DubSdlParser` | `dependency "name" version="..."` lines |
| `stack.yaml` | Haskell / Stack | `stack::StackParser` | `extra-deps:` list; `pkg-name-1.2.3` → name + version |
| `flake.nix` | Nix | `flake::FlakeParser` | `inputs.<name>` (name only; no semver constraints in Nix) |
| `Package.swift` | Swift / SPM | `swift_pm::SwiftPmParser` | `.package(url:, from:)` calls; URL → name; version forms mapped |

## Supported — Extension-Based

Dispatched by `parse_manifest_by_extension(ext, content)` or via `parse_manifest(filename, content)`
when the filename has a recognized extension.

| Extension | Ecosystem | Parser | Extracts |
|---|---|---|---|
| `*.nimble` | Nim / Nimble | `nimble::NimbleParser` | `requires "pkg >= 1.0"` lines; Nim runtime filtered |
| `*.cabal` | Haskell / Cabal | `cabal::CabalParser` | `build-depends:` field; `base` filtered; test-suite → Dev |
| `*.csproj` / `*.vbproj` / `*.fsproj` | .NET / NuGet | `nuget::CsprojParser` | `<PackageReference>` elements; `PrivateAssets="all"` → Dev |
| `*.rockspec` | Lua / LuaRocks | `rockspec::RockspecParser` | `dependencies = { ... }` table; Lua runtime filtered |

## Convenience Helpers

Exposed at crate root:
- `go_module(content)` — used by `normalize-local-deps` to extract module info from `go.mod`
- `npm_entry_point(content)` — used by `normalize-local-deps` to find the entry point in `package.json`

## Not Yet Supported

| Manifest | Ecosystem | Notes |
|---|---|---|
| `setup.py` | Python | Python source file; `setup(install_requires=[...])` requires execution to evaluate |

## Parser Notes

### requirements.txt gaps
Does NOT handle: `-r` includes, URL deps (`git+https://...`), path deps (`./local/pkg`),
environment markers (`; python_version >= "3.9"`) — silently skipped.

### build.gradle / build.gradle.kts
Extracts deps from `dependencies { ... }` block. Configuration name → DepKind:
- `implementation`, `api`, `compileOnly`, `runtimeOnly` → Normal
- `testImplementation`, `testCompileOnly`, `testRuntimeOnly`, etc. → Dev

Does not handle: version catalogs (`libs.somelib`), platform declarations, BOM imports.

### pom.xml
Skips `<dependencyManagement>` (version constraints, not direct deps).
`<scope>test</scope>` → Dev; `<scope>provided</scope>` or `<optional>true</optional>` → Optional.

### pubspec.yaml
`sdk:` sub-keys (e.g. `sdk: flutter`) are discarded — platform dep, not a package.
`git:` / `path:` sub-keys push the dep with `version_req: None`.

### flake.nix
`nixpkgs` filtered (near-universal). Only input names are returned — Nix has no semver
constraints so `version_req` is always `None`. Source URL not preserved in `DeclaredDep`.

### *.csproj
`PrivateAssets="all"` → Dev (covers analyzers, test adapters, build tooling).
Also handles `.vbproj` and `.fsproj` (same XML schema).

## Adding a New Parser

1. Add `src/<name>.rs` with a struct implementing `ManifestParser`
2. Re-export from `src/lib.rs` (`pub mod <name>;`)
3. Add to `parse_manifest()` match arm (or `parse_manifest_by_extension_impl()` for wildcards)
4. Add unit tests in the parser file
5. Update this table
