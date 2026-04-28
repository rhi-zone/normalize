; Rust test regions query
;
; Captures regions of Rust source that are test-only code. Findings inside
; any captured @test_region range are skipped by the syntax-rules runner
; for rules that have not opted in via `applies_in_tests = true` in their
; frontmatter.
;
; Currently captures inline `#[cfg(test)] mod ... { ... }` blocks — the
; dominant Rust convention for unit tests, which path-based `**/tests/**`
; allow globs do not catch.

(mod_item
  (attributes
    (attribute_item
      (attribute
        (identifier) @_attr_name
        (token_tree (identifier) @_arg)
        (#eq? @_attr_name "cfg")
        (#eq? @_arg "test"))))) @test_region
