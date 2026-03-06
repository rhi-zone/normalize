# fixtures/no-todo-comment

Test fixtures for the cross-language `no-todo-comment` rule.

Contains `match.rs` (source with a `TODO` comment — expected to produce a finding) and `no_match.rs` (source with no TODO comments — expected to produce zero findings). This rule applies across all supported languages; the fixture uses Rust as a representative case.
