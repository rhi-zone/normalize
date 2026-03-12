# ---
# id = "c/magic-number"
# severity = "info"
# tags = ["readability"]
# message = "Magic number in comparison - extract to a named constant"
# languages = ["c", "cpp"]
# enabled = false
# ---
#
# Magic numbers are numeric literals whose meaning is not obvious from
# context. Extracting them to named constants (or `#define` / `enum`
# in C) makes the code self-documenting.
#
# ## How to fix
#
# ```c
# // Before
# if (retries > 3) { ... }
# // After
# #define MAX_RETRIES 3
# if (retries > MAX_RETRIES) { ... }
# ```
#
# ## When to disable
#
# Disabled by default. Ignores single-digit values (0-9).

; number_literal covers int, float, hex, octal in tree-sitter-c
((binary_expression
  operator: [">" "<" ">=" "<=" "==" "!="]
  right: (number_literal) @_num) @match
 (#not-match? @_num "^[0-9]$"))

((binary_expression
  left: (number_literal) @_num
  operator: [">" "<" ">=" "<=" "==" "!="]) @match
 (#not-match? @_num "^[0-9]$"))
