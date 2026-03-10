# ---
# id = "go/empty-return"
# severity = "warning"
# tags = ["style", "cleanup"]
# message = "Bare `return` at end of void function is unnecessary"
# languages = ["go"]
# enabled = false
# ---
#
# In Go, a function with no return values exits cleanly when execution
# reaches the closing `}`. A bare `return` at the very end is a no-op
# that adds visual noise without conveying any information to the reader.
#
# Named-return functions with `return` on the last line *do* benefit from
# the explicit form, but they have a result type in their signature and are
# excluded from this rule.
#
# ## How to fix
#
# Delete the trailing `return` statement.
#
# ## When to disable
#
# This rule is disabled by default (warning severity). Some teams prefer
# explicit `return` at the end of all functions for visual consistency.
# Disable globally if that is your convention.

; Bare return as the last statement in a void function body.
; `!result` matches function_declarations without a return type.
; tree-sitter-go: block → statement_list → statements
(function_declaration
  !result
  body: (block
    (statement_list
      (return_statement) @match .)))

(method_declaration
  !result
  body: (block
    (statement_list
      (return_statement) @match .)))
