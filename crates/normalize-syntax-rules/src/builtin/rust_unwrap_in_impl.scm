# ---
# id = "rust/unwrap-in-impl"
# severity = "info"
# tags = ["error-handling"]
# message = ".unwrap() found - consider using ? or .expect() with context"
# languages = ["rust"]
# allow = ["**/tests/**", "**/test_*.rs", "**/*_test.rs", "**/*_tests.rs", "**/examples/**", "**/benches/**"]
# enabled = false
# ---
#
# `.unwrap()` panics on `None` or `Err` — in production code, an unexpected
# `None` or error crashes the entire process with no opportunity for the
# caller to recover. Every `.unwrap()` is an unhandled error case.
#
# ## How to fix
#
# Use `?` to propagate errors to the caller, `.unwrap_or_else(|e| ...)` to
# provide a fallback, or `.expect("context message")` to make the panic
# informative. Prefer `?` in most cases.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Test code legitimately
# uses `.unwrap()` for brevity — test files are already excluded in the
# default allow list.

; Detects: .unwrap() calls
((call_expression
  function: (field_expression
    field: (field_identifier) @_method)
  (#eq? @_method "unwrap")) @match)
