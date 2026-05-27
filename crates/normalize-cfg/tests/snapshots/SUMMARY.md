# snapshots

Insta snapshot files for `normalize-cfg` CFG (control-flow graph) tests.

Each `.snap` file captures the expected CFG output for a language-specific test case
(Rust, Python, Go, Java, TypeScript, JavaScript, Jinja2, Lua, and others). Generated
and updated via `cargo insta review` / `INSTA_UPDATE=always cargo test`.
