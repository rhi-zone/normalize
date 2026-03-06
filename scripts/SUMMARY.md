# Scripts

Shell scripts for development and maintenance tasks. `pre-commit` is the git pre-commit hook — it runs `cargo fmt --check`, `cargo clippy`, and normalize's own checks (summary enforcement, syntax rules) before each commit. `missing-grammars.sh` queries the crates.io API to find arborium language features not yet implemented in `normalize-languages`. `session-corrections.sh` extracts correction patterns from Claude Code session logs to surface candidates for new `CLAUDE.md` rules.
