# ---
# id = "js/typeof-string"
# severity = "info"
# tags = ["style", "correctness"]
# message = "`typeof x ==` uses loose equality - use `===` for strict type comparison"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# `typeof x == 'string'` uses loose (`==`) equality, which triggers type
# coercion. While `typeof` always returns a string so coercion is harmless
# here, the inconsistency invites confusion: readers may not know whether
# the coercion is intentional, and linters like ESLint's `eqeqeq` rule will
# flag it regardless.
#
# Using `===` is idiomatic JavaScript, signals intent clearly, and avoids
# any accidental coercion if the pattern is copy-pasted into a context where
# it matters.
#
# ## How to fix
#
# Replace `==` with `===`:
#
# ```js
# if (typeof x === 'string') { ... }
# ```
#
# ## When to disable
#
# This rule is disabled by default (info severity). There is no valid reason
# to use `==` instead of `===` when comparing `typeof` output, but if you
# need to suppress the diagnostic for a specific line, use an inline comment.

; Detects: typeof x == expr — loose equality on a typeof expression
(binary_expression
  left: (unary_expression operator: "typeof")
  operator: "==") @match
