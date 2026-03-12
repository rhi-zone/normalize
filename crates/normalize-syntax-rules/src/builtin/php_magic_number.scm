# ---
# id = "php/magic-number"
# severity = "info"
# tags = ["readability"]
# message = "Magic number in comparison - extract to a named constant"
# languages = ["php"]
# enabled = false
# ---
#
# Magic numbers are numeric literals whose meaning is not obvious from
# context. Extracting them to named constants makes the code
# self-documenting and values easy to change consistently.
#
# ## How to fix
#
# ```php
# // Before
# if ($retries > 3) { ... }
# // After
# const MAX_RETRIES = 3;
# if ($retries > MAX_RETRIES) { ... }
# ```
#
# ## When to disable
#
# Disabled by default (info severity). Ignores single-digit values (0-9)
# which are usually obvious from context.

; Matches integer literals in binary comparison expressions,
; excluding single-digit numbers (0-9).
((binary_expression
  operator: [">" "<" ">=" "<=" "==" "!=" "===" "!=="]
  right: (integer) @_num) @match
 (#not-match? @_num "^[0-9]$"))

((binary_expression
  left: (integer) @_num
  operator: [">" "<" ">=" "<=" "==" "!=" "===" "!=="]) @match
 (#not-match? @_num "^[0-9]$"))
