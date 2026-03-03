; Scopes
; ------

[
  (function_definition)
  (method_declaration)
  (anonymous_function)
  (arrow_function)
  (compound_statement)
  (for_statement)
  (foreach_statement)
] @local.scope

; Definitions
; -----------

; Function names
(function_definition
  name: (name) @local.definition)

; Method names
(method_declaration
  name: (name) @local.definition)

; Function and method parameters ($var)
(simple_parameter
  name: (variable_name) @local.definition)

; Variadic parameters ($var...)
(variadic_parameter
  name: (variable_name) @local.definition)

; References
; ----------

; PHP variables are $name — capture the variable_name node.
; Its text includes the $ sigil, which consistently identifies variables.
(variable_name) @local.reference
