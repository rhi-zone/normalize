# normalize-tools/src/test_runners

Test runner adapters for detecting and invoking ecosystem-native test commands.

Implements the `TestRunner` trait for five ecosystems (all feature-gated): `cargo.rs` (`CargoTest` — `cargo test`), `go.rs` (`GoTest` — `go test ./...`), `pytest.rs` (`Pytest` — `pytest`), `bun.rs` (`BunTest` — `bun test`), `npm.rs` (`NpmTest` — `npm test`). Each runner detects relevance by checking project files (Cargo.toml, go.mod, pyproject.toml, package.json), streams test output directly to stdout/stderr, and returns a `TestResult` with the process `ExitStatus`. `detect_test_runner` picks the highest-confidence available runner for a project root.
