# ---
# id = "go/context-todo"
# severity = "warning"
# tags = ["correctness", "style"]
# message = "`context.TODO()` is a placeholder — replace with a real context threaded from the caller"
# languages = ["go"]
# allow = ["**/main.go", "**/cmd/**", "**/examples/**", "**/*_test.go", "**/tests/**"]
# enabled = false
# ---
#
# `context.TODO()` is an explicitly temporary placeholder that signals
# "I know I need a real context here but haven't threaded one through yet."
# It is semantically equivalent to `context.Background()` at runtime, but
# its name is a documented signal of incomplete work.
#
# Leaving `context.TODO()` in production library code means:
# - Callers cannot cancel or set deadlines on the operation.
# - Timeouts and tracing signals from the caller are silently dropped.
# - The codebase has unfulfilled context threading debt.
#
# ## How to fix
#
# Thread `ctx context.Context` as the first parameter of the enclosing
# function and pass it down:
#
# ```go
# // Before:
# func FetchUser(id string) (*User, error) {
#     return db.QueryContext(context.TODO(), "SELECT ...", id)
# }
#
# // After:
# func FetchUser(ctx context.Context, id string) (*User, error) {
#     return db.QueryContext(ctx, "SELECT ...", id)
# }
# ```
#
# Use `context.Background()` in entry points (`main`, top-level servers,
# test setup) where there is genuinely no parent context.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). `main.go`, `cmd/`,
# and test files are excluded by default. For active migration work, use the
# allow list to exclude files you have not yet threaded.

; context.TODO() call
((call_expression
  function: (selector_expression
    operand: (identifier) @_pkg
    field: (field_identifier) @_method)
  (#eq? @_pkg "context")
  (#eq? @_method "TODO")) @match)
