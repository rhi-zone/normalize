# Normalize Monorepo

Normalize is a polyglot code intelligence CLI toolchain providing structural analysis of codebases through AST-based extraction. The monorepo contains 30+ Rust crates (published on crates.io), a VS Code extension, a web sessions viewer, documentation site, and supporting tooling. The main `normalize` binary consumes a library ecosystem of domain crates covering language support, facts extraction, rules evaluation, manifest parsing, and output formatting. Core development conventions are in `CLAUDE.md`; architecture decisions, design philosophy, and CLI documentation live under `docs/`.

The `normalize-syntax-rules` crate provides the syntax linting engine; builtin rules cover Rust, JS/TS, Python, Go, and Ruby with fix-fixture infrastructure for testing auto-fix transforms.

Every directory with source files has a `SUMMARY.md` — enforced at `severity=error` via `normalize analyze check --summary` in the pre-commit hook.
