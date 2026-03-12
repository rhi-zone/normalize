# ---
# id = "java/magic-number"
# severity = "info"
# tags = ["readability"]
# message = "Magic number in comparison - extract to a named constant"
# languages = ["java"]
# enabled = false
# ---
#
# Magic numbers are numeric literals whose meaning is not obvious from
# context. Extracting them to named constants makes the code
# self-documenting and values easy to change consistently.
#
# ## How to fix
#
# ```java
# // Before
# if (retries > 3) { ... }
# // After
# private static final int MAX_RETRIES = 3;
# if (retries > MAX_RETRIES) { ... }
# ```
#
# ## When to disable
#
# Disabled by default (info severity). Ignores single-digit values (0-9)
# which are usually obvious from context. Enable if you want to enforce
# a no-magic-numbers policy.

; Matches integer literals in binary comparison expressions,
; excluding single-digit numbers (0-9).
((binary_expression
  operator: [">" "<" ">=" "<=" "==" "!="]
  right: (decimal_integer_literal) @_num) @match
 (#not-match? @_num "^[0-9]$"))

((binary_expression
  left: (decimal_integer_literal) @_num
  operator: [">" "<" ">=" "<=" "==" "!="]) @match
 (#not-match? @_num "^[0-9]$"))
