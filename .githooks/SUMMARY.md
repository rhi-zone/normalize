# .githooks/

Project-scoped git hooks. Activated by setting `core.hooksPath` to this directory:
`git config core.hooksPath .githooks`.

`pre-commit` — runs `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `normalize rules run` against the staged tree before allowing a commit.
