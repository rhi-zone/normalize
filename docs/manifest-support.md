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
`setup.py` — not `requirements.txt`.

## Priority Order

1. `pom.xml` — high impact (Java + Kotlin + Scala share it); XML, `quick-xml` or `roxmltree`
2. `build.gradle` / `build.gradle.kts` — used by same three ecosystems; Groovy/Kotlin DSL
   parsing is non-trivial (regex over known patterns is practical)
3. `setup.cfg` — simpler INI format; fills Python gap for legacy projects
4. `build.sbt` — Scala-only; line-pattern matching on `libraryDependencies` is practical
5. `setup.py` — lowest priority; requires executing Python or best-effort regex

## Adding a New Parser

1. Add `src/<name>.rs` with a struct implementing `ManifestParser`
2. Re-export from `src/lib.rs` and add to `parse_manifest()` match arm
3. Add unit tests in the parser file
4. Update this table
