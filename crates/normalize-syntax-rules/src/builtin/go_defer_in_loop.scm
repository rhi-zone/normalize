# ---
# id = "go/defer-in-loop"
# severity = "warning"
# tags = ["bug-prone", "correctness"]
# message = "`defer` inside a loop runs at function return, not end of iteration"
# languages = ["go"]
# enabled = false
# ---
#
# `defer` schedules a call to run when the *enclosing function* returns,
# not when the current loop iteration ends. Placing `defer` inside a loop
# accumulates all deferred calls until the function exits, which means:
#
# - Resources opened per-iteration (files, connections, mutexes) are not
#   released until the function returns, potentially exhausting them.
# - The order of deferred calls is LIFO: the last defer runs first.
#
# ## How to fix
#
# Extract the loop body into a helper function and call `defer` inside it:
#
# ```go
# for _, path := range paths {
#     if err := processFile(path); err != nil {
#         return err
#     }
# }
#
# func processFile(path string) error {
#     f, err := os.Open(path)
#     if err != nil {
#         return err
#     }
#     defer f.Close()  // runs when processFile returns
#     // ...
# }
# ```
#
# ## When to disable
#
# This rule is disabled by default (warning severity). If the defer is for
# cleanup that should accumulate (e.g., deferring a single cancel function
# that is valid for the entire function lifetime), use an allow comment.
# Defers inside immediately-invoked function literals inside a loop are
# a valid workaround and will not be flagged by this rule.

; Direct defer_statement as a named child of a for_statement's block.
; Defers inside nested function literals (func() { defer ... }()) are
; excluded because the literal creates a new function scope.
; tree-sitter-go: block → statement_list → statements
(for_statement
  body: (block
    (statement_list
      (defer_statement) @match)))
