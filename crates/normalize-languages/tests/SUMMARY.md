# normalize-languages/tests

Integration tests for the normalize-languages crate.

- `query_fixtures.rs` — fixture-based tests that parse sample source files, run the five `.scm` query types (tags, calls, complexity, imports, types), and assert specific captures appear. Tests skip gracefully when grammar `.so` files are not present (`target/grammars/` absent). Build grammars with `cargo xtask build-grammars` to run them with actual grammar execution.
- `fixtures/` — small representative source files (30-60 lines each) used by `query_fixtures.rs`, one per language: Rust, Python, Go, TypeScript, Java, Ruby, Kotlin, Swift, Scala, PHP, Dart, Elixir, C, C++, C#, Haskell, OCaml, Erlang, Lua, Groovy, Julia, R, F#, Gleam.
