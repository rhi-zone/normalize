; Scopes
; ------

[
  (function_definition)
  (compound_statement)
  (subshell)
] @local.scope

; Definitions
; -----------

; Function names
(function_definition
  name: (word) @local.definition)

; Variable assignments
(variable_assignment
  name: (variable_name) @local.definition)

; For loop variable
(for_statement
  variable: (variable_name) @local.definition)

; References
; ----------

(variable_name) @local.reference
