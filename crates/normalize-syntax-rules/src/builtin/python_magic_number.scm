# ---
# id = "python/magic-number"
# severity = "info"
# tags = ["readability"]
# message = "Magic number in comparison - extract to a named constant"
# languages = ["python"]
# enabled = false
# ---
#
# Magic numbers are numeric literals whose meaning is not obvious from
# context. When a comparison like `if retries > 3` appears in code, the
# reader must guess what `3` represents. Extracting it to
# `MAX_RETRIES = 3` makes the code self-documenting and the value easy to
# change consistently.
#
# ## How to fix
#
# Extract the number to a module-level constant with a descriptive
# UPPER_SNAKE_CASE name:
#
# ```python
# MAX_RETRIES = 3
# if retries > MAX_RETRIES:
# ```
#
# ## When to disable
#
# This rule is disabled by default (info severity). It only flags numbers
# in comparison expressions and ignores single-digit values (0-9).
# Enable it if you want to enforce a no-magic-numbers policy.

; Matches integer literals in comparison expressions, excluding single-digit
; numbers (0-9) which are usually obvious from context.
((comparison_operator
  (integer) @_num) @match
 (#not-match? @_num "^[0-9]$"))
