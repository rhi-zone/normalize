# ---
# id = "rust/magic-number"
# severity = "info"
# tags = ["readability"]
# message = "Magic number in comparison - extract to a named constant"
# languages = ["rust"]
# enabled = false
# ---
#
# Magic numbers are numeric literals whose meaning is not obvious from
# context. When a comparison like `if retries > 3` appears in code, the
# reader must guess what `3` represents. Extracting it to
# `const MAX_RETRIES: usize = 3` makes the code self-documenting and the
# value easy to change consistently.
#
# ## How to fix
#
# Extract the number to a named constant:
#
# ```rust
# const MAX_RETRIES: usize = 3;
# if retries > MAX_RETRIES {
# ```
#
# ## When to disable
#
# This rule is disabled by default (info severity). It only flags numbers
# in comparison expressions and ignores single-digit values (0-9).
# Enable it if you want to enforce a no-magic-numbers policy.

; Matches integer literals in binary comparison expressions, excluding
; single-digit numbers (0-9) which are usually obvious from context.
((binary_expression
  operator: [">" "<" ">=" "<=" "==" "!="]
  right: (integer_literal) @_num) @match
 (#not-match? @_num "^[0-9]$"))

; Also match when the number is on the left side
((binary_expression
  left: (integer_literal) @_num
  operator: [">" "<" ">=" "<=" "==" "!="]) @match
 (#not-match? @_num "^[0-9]$"))
