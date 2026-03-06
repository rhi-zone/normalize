# fixtures/hardcoded-secret

Test fixtures for the cross-language `hardcoded-secret` rule.

Contains `match.rs` (code that assigns a string literal to a variable named `secret`, `password`, `api_key`, etc. — expected to produce findings) and `no_match.rs` (code that does not trigger the rule). This rule applies across all supported languages; the fixture uses Rust as a representative case.
