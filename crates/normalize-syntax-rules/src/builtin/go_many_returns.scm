# ---
# id = "go/many-returns"
# severity = "info"
# tags = ["style"]
# message = "Function has 3+ return values - consider using a struct"
# languages = ["go"]
# enabled = false
# ---
#
# Go functions that return three or more values are doing too much or
# returning results that belong together in a struct. Beyond the idiomatic
# `(value, error)` or `(value, bool)` pairs, positional return values
# become ambiguous and callers must use multiple assignment with positional
# names that carry no type-enforced meaning.
#
# ## How to fix
#
# Define a result struct and return it. Named fields make call sites
# self-documenting and allow the result shape to evolve without changing
# every caller.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Parsing or scanning
# functions that return `(start, end, value, error)` may be a legitimate
# exception when the positions are well-understood.

; Detects functions with 3 or more return values
; More than (value, error) usually warrants a result struct
(function_declaration
  result: (parameter_list
    (parameter_declaration)
    (parameter_declaration)
    (parameter_declaration)) @match)
