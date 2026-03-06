# normalize-tools

Unified interface for external development tools (linters, formatters, type checkers, test runners).

Wraps tools like oxlint, eslint, ruff, mypy, pyright, prettier, biome, clippy, rustfmt, tsc, tsgo, gofmt, and deno behind a common `Tool` trait that provides availability detection (`is_available`), project relevance scoring (`detect`), and diagnostic output (`run` → `ToolResult` with `Vec<Diagnostic>`). A global `ToolRegistry` discovers and runs relevant tools in parallel. Custom tools can be added via `.normalize/tools.toml` (loaded by `load_custom_tools`) or registered programmatically. Also exposes a `test_runners` module with a `TestRunner` trait covering cargo, go test, pytest, bun, and npm test. All tools and test runners are individually feature-gated.
