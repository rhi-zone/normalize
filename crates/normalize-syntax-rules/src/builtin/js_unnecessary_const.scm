# ---
# id = "js/unnecessary-const"
# severity = "info"
# tags = ["style"]
# message = "Unnecessary const binding - consider using the value directly"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# enabled = false
# ---
#
# `const x = y;` where both sides are simple identifiers rebinds a name
# without adding transformation or meaning. The reader now has to track two
# names for the same value, and the extra binding obscures the flow of data.
#
# ## How to fix
#
# Use the original name directly. If the alias adds clarity at a scope
# boundary (e.g., destructuring a long expression into a short name), the
# pattern is intentional — disable for that file.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Intentional aliasing for
# readability is a legitimate use.

; Detects: const x = y; where both are simple identifiers
; Excludes: undefined, Infinity, NaN (global constants)
((lexical_declaration
  kind: "const"
  (variable_declarator
    name: (identifier) @_alias
    value: (identifier) @_value))
  (#not-any-of? @_value "undefined" "Infinity" "NaN")) @match
