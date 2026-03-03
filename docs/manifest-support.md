# Manifest Support

Coverage status for the `normalize-manifest` crate.

Each row is a manifest file format. "Parser" = a `ManifestParser` impl exists in
`normalize-manifest`. "Extracts" = what that parser currently returns.

## Supported

| Manifest | Ecosystem | Parser | Extracts |
|---|---|---|---|
| `Cargo.toml` | Rust / Cargo | `cargo::CargoParser` | name, version, `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` |
| `go.mod` | Go | `go_mod::GoModParser` | module path (→ name), go version, `require` directives |
| `package.json` | npm / Node.js | `npm::NpmParser` | name, version, `dependencies`, `devDependencies`, `peerDependencies` |
| `requirements.txt` | pip | `pip::PipParser` | deps with version specifiers (one per line) |
| `pyproject.toml` | Python (PEP 621 + Poetry) | `pyproject::PyprojectParser` | name, version, `[project.dependencies]`, `[tool.poetry.dependencies]`, `[tool.poetry.dev-dependencies]` |

Convenience helpers exposed at crate root:
- `go_module(content)` — used by `normalize-local-deps` instead of `parse_go_mod_content()`
- `npm_entry_point(content)` — used by `normalize-local-deps` instead of `get_package_entry_point()`

## Not Yet Supported

Formats referenced by `normalize-local-deps` `project_manifest_filenames()` that have no parser:

| Manifest | Ecosystem | Notes |
|---|---|---|
| `pom.xml` | Java / Maven | XML; used by Java, Kotlin, Scala impls. Declares `<dependencies>` with `groupId`/`artifactId`/`version`. |
| `build.gradle` | Java / Gradle (Groovy DSL) | Groovy DSL; used by Java, Kotlin, Scala. `implementation`, `testImplementation`, etc. |
| `build.gradle.kts` | Java / Gradle (Kotlin DSL) | Kotlin DSL variant of above; similar structure. |
| `build.sbt` | Scala / sbt | Custom DSL; `libraryDependencies += ...` syntax. |
| `setup.py` | Python | `setup(install_requires=[...])` — Python file, not trivially parseable without execution. |
| `setup.cfg` | Python | INI-style; `[options] install_requires = ...` |

Python `normalize-local-deps` also lists `requirements.txt` indirectly (some projects use it),
but the `project_manifest_filenames()` for Python currently returns `pyproject.toml`, `setup.cfg`,
`setup.py` — not `requirements.txt`. Add `requirements.txt` to that list once confirmed.

`requirements.txt` is also widely used as a supplementary dep file even in projects that have
`pyproject.toml` (e.g., `requirements-dev.txt`, `requirements-lock.txt`). The parser handles the
common `name==version`, `name>=version`, bare `name` forms but not: `-r includes`, URL deps
(`git+https://...`), path deps (`./local/pkg`), or environment markers (`; python_version >= "3.9"`
after the version spec). Those are skipped silently.

## Niche / Heuristic-only

Formats where full parsing is impractical (dynamic files, custom DSLs, non-standard filenames)
but heuristic extraction of deps is possible and useful:

| Manifest | Ecosystem | Filename | Parsing approach |
|---|---|---|---|
| Nimble | Nim | `*.nimble` | Nim source file. `requires "pkg >= 1.0"` lines are line-pattern matchable. |
| Conan v1 | C / C++ | `conanfile.txt` | INI-style `[requires]` section; straightforward. |
| Conan v2 | C / C++ | `conanfile.py` | Python file; heuristic regex over `requires = [...]` or `self.requires(...)`. |
| Nix flake | Nix | `flake.nix` | Nix expression; full eval not feasible. Extract `inputs.<name>.url` by line-pattern matching — gives dep names, not version reqs. |
| Gemfile | Ruby / Bundler | `Gemfile` | Ruby DSL; `gem "name", "~> 1.0"` lines are line-pattern matchable. |
| `composer.json` | PHP / Composer | `composer.json` | JSON (`require`, `require-dev`); trivially parseable. |
| `pubspec.yaml` | Dart / Flutter | `pubspec.yaml` | YAML (`dependencies:`, `dev_dependencies:`); needs `serde_yaml` or line parsing. |
| `Package.swift` | Swift / SPM | `Package.swift` | Swift file; `.package(url:, from:)` calls are regex-matchable. |
| `*.cabal` | Haskell / Cabal | `*.cabal` | Custom format; `build-depends:` field, comma-separated. Line-pattern matchable. |
| `stack.yaml` | Haskell / Stack | `stack.yaml` | YAML; `extra-deps:` list. |
| `mix.exs` | Elixir / Hex | `mix.exs` | Elixir file; `{:pkg, "~> 1.0"}` tuples in `deps/0` — regex-matchable. |
| `dub.json` / `dub.sdl` | D / Dub | `dub.json`, `dub.sdl` | JSON or custom SDL; `"dependencies"` object. |
| `*.csproj` | .NET / NuGet | `*.csproj` | XML; `<PackageReference Include="..." Version="..."/>`. |
| `packages.config` | .NET / NuGet (legacy) | `packages.config` | XML; `<package id="..." version="..."/>`. Older projects before SDK-style csproj. |
| `*.rockspec` | Lua / LuaRocks | `*.rockspec` | Lua file; `dependencies = { "pkg >= 1.0" }` table — string list, regex-matchable. Non-standard filename (contains version: `pkg-1.0-1.rockspec`). |

**.sln is not a manifest.** Visual Studio solution files list which `.csproj`/`.vbproj`/`.fsproj`
projects belong to a solution (GUIDs + paths), but contain no dependency declarations. To get .NET
deps: find `*.sln` → extract referenced project file paths → parse each `*.csproj`. The `.sln`
itself is only useful as a project enumerator, not a dep source.

Notes:
- **Nimble** and **Gemfile** are the most tractable (simple line patterns).
- **Nix flake** can only yield input names and URLs, not semver constraints — record as
  `version_req: None`. The `url` often encodes a rev or tag which could be stored as-is.
- **Conan v2** and **Package.swift** are Python/Swift files; regex over known call patterns is
  the only option without a full language parser.
- **`*.cabal`** and **`*.csproj`** use non-standard filename patterns — `parse_manifest()` can't
  dispatch by exact filename; callers must detect the extension first.

## Priority Order

1. `pom.xml` — high impact (Java + Kotlin + Scala share it); XML, `quick-xml` or `roxmltree`
2. `build.gradle` / `build.gradle.kts` — used by same three ecosystems; Groovy/Kotlin DSL
   parsing is non-trivial (regex over known patterns is practical)
3. `composer.json` — trivial (JSON, same shape as `package.json`)
4. `conanfile.txt` — trivial (INI `[requires]` section)
5. `pubspec.yaml` — YAML, needs a dep or line parsing
6. `setup.cfg` — INI-style, fills Python gap for legacy projects
7. `Gemfile` — line-pattern, high ecosystem value (Ruby)
8. `mix.exs` — regex, high ecosystem value (Elixir)
9. `build.sbt` — Scala-only; line-pattern matching on `libraryDependencies` is practical
10. `flake.nix` — heuristic only; yields input names, no version reqs
11. `*.nimble`, `stack.yaml`, `dub.json`, `Package.swift`, `*.cabal`, `*.csproj` — lower priority or awkward filename dispatch

## Adding a New Parser

1. Add `src/<name>.rs` with a struct implementing `ManifestParser`
2. Re-export from `src/lib.rs` and add to `parse_manifest()` match arm
3. Add unit tests in the parser file
4. Update this table
