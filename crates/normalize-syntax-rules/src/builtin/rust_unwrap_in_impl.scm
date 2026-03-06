# ---
# id = "rust/unwrap-in-impl"
# severity = "info"
# tags = ["error-handling"]
# message = ".unwrap() found - consider using ? to propagate or .unwrap_or_else() for fallbacks"
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
# **Preferred:** Use `?` to propagate errors/None to the caller. This requires
# the function to return `Result` or `Option` — if it doesn't, consider whether
# it should. Callers should handle errors, not panic on them.
#
# **For fallbacks:** `.unwrap_or(default)`, `.unwrap_or_else(|| compute())`,
# `.unwrap_or_default()`, or `if let Some(x) = ...`.
#
# **Avoid:** `.expect("message")` — it is still a panic, just a louder one.
# Use it only for conditions that are genuinely impossible (compile-time
# invariants), and add a comment explaining why it cannot fail.
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
