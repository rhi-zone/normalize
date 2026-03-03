; Scopes
; ------

[
  (compound_statement)
  (function_definition)
  (lambda_expression)
  (for_statement)
  (for_range_loop)
  (if_statement)
  (while_statement)
  (do_statement)
  (switch_statement)
  (catch_clause)
] @local.scope

; Definitions
; -----------

; Function names (via declarator chain)
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @local.definition))

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @local.definition)))

; Parameters
(parameter_declaration
  declarator: (identifier) @local.definition)

(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @local.definition))

(parameter_declaration
  declarator: (reference_declarator
    (identifier) @local.definition))

; Local variable declarations (simple)
(declaration
  declarator: (identifier) @local.definition)

; Local variable declarations with initializer
(init_declarator
  declarator: (identifier) @local.definition)

; Range-based for loop variable
(for_range_loop
  declarator: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
