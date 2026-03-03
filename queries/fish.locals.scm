; Fish shell locals.scm
; Fish has function-level scoping. The function name is a `word` child
; of function_definition (no named field). For loop variables use the
; distinct `variable_name` node kind. References use variable_expansion.

; Scopes
; ------

(function_definition) @local.scope

; Definitions
; -----------

; Function name: first word after the "function" keyword
(function_definition
  "function" .
  (word) @local.definition)

; For loop variable (variable_name is a distinct node kind)
(for_statement
  (variable_name) @local.definition)

; References
; ----------

; Variable references: $varname
(variable_expansion
  (variable_name) @local.reference)
