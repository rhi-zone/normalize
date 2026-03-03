; Scopes
; ------

[
  (block)
  (method_declaration)
  (constructor_declaration)
  (lambda_expression)
  (anonymous_method_expression)
  (for_statement)
  (foreach_statement)
  (if_statement)
  (while_statement)
  (do_statement)
  (try_statement)
  (catch_clause)
  (switch_section)
  (local_function_statement)
] @local.scope

; Definitions
; -----------

; Method names
(method_declaration
  name: (identifier) @local.definition)

; Constructor names
(constructor_declaration
  name: (identifier) @local.definition)

; Local function names
(local_function_statement
  name: (identifier) @local.definition)

; Parameters
(parameter
  name: (identifier) @local.definition)

; Local variable declarations
(variable_declarator
  name: (identifier) @local.definition)

; Foreach loop variable
(foreach_statement
  left: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
