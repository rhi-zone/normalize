# ---
# id = "go/error-ignored"
# severity = "info"
# tags = ["error-handling"]
# message = "`_ =` discards function return - if this is an error, handle it"
# languages = ["go"]
# enabled = false
# ---
#
# `_ = someFunc()` silently discards all return values from a function call.
# When the function returns an `error`, ignoring it means the caller has no
# way to react to failures — the program continues as if the call succeeded.
# This is a common source of silent data loss, resource leaks, and hard-to-
# diagnose bugs.
#
# ## How to fix
#
# Capture the return values and check the error:
#
# ```go
# if err := someFunc(); err != nil {
#     return fmt.Errorf("someFunc: %w", err)
# }
# ```
#
# If you genuinely intend to discard the return (e.g., `_ = buf.Write(b)` for
# a write you know cannot fail), add a comment explaining why, or disable this
# rule for that file.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Certain stdlib functions
# (`fmt.Fprintf` to a discard writer, `io.Copy` for draining) are commonly
# discarded. Use the allow list or a per-line disable comment.

; Detects: _ = someFunc() — entire function return discarded to blank identifier
; In tree-sitter-go, _ in an assignment left-hand side is an identifier with text "_"
(assignment_statement
  left: (expression_list . (identifier) @_blank .)
  right: (expression_list . (call_expression) .)
  (#eq? @_blank "_")) @match
