# ---
# id = "js/var-declaration"
# severity = "info"
# tags = ["style"]
# message = "`var` declaration - use `let` or `const` instead"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# `var` has function scope and hoisting behavior that leads to subtle bugs.
# Variables declared with `var` are hoisted to the top of their enclosing
# function (or the global scope), meaning they can be accessed before the line
# where they are declared. They also leak out of `if`, `for`, and other blocks,
# since those blocks do not create a new scope for `var`.
#
# `let` and `const` have block scope, are not hoisted in a usable way (the
# temporal dead zone throws a ReferenceError), and do not leak out of blocks.
# This makes the code easier to reason about and eliminates an entire class of
# accidental-reuse bugs.
#
# ## How to fix
#
# - Use `const` if the binding is never reassigned.
# - Use `let` if the binding needs to be reassigned.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Legacy codebases with many
# `var` declarations may want to enable it incrementally. If a file must be
# compatible with very old JavaScript environments that lack `let`/`const`,
# disable per file.

; Detects var declarations — function-scoped and hoisted, prefer let or const
(variable_declaration "var" @_kw) @match
