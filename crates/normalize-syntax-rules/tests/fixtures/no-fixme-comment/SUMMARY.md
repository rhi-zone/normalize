# fixtures/no-fixme-comment

Test fixtures for the cross-language `no-fixme-comment` rule.

Contains `match.rs` (source with a `FIXME` comment — expected to produce a finding) and `no_match.rs` (source with no FIXME comments — expected to produce zero findings). This rule applies across all supported languages; the fixture uses Rust as a representative case.
