# normalize-ecosystems

Project dependency management for multiple package ecosystems.

Defines the `Ecosystem` trait for detecting, querying, and auditing project dependencies. Provides types `PackageInfo`, `Dependency`, `DepSource`, `DependencyTree`, `Vulnerability`, and `AuditResult`. Built-in implementations cover 12 ecosystems (cargo, npm, deno, python, go, hex, gem, composer, maven, nuget, nix, conan) selected via Cargo features, plus a global plugin registry (`detect_ecosystem`, `get_ecosystem`, `register_ecosystem`). The `query()` method on `Ecosystem` uses a 24-hour on-disk cache and falls back to stale cache on network failure.
