; Scopes
; ------

[
  (block)
  (method_declaration)
  (constructor_declaration)
  (lambda_expression)
  (enhanced_for_statement)
  (for_statement)
  (if_statement)
  (while_statement)
  (do_statement)
  (try_statement)
  (catch_clause)
  (switch_block_statement_group)
] @local.scope

; Definitions
; -----------

; Method names
(method_declaration
  name: (identifier) @local.definition)

; Constructor names
(constructor_declaration
  name: (identifier) @local.definition)

; Formal parameters
(formal_parameter
  name: (identifier) @local.definition)

; Spread (vararg) parameters
(spread_parameter
  (variable_declarator
    name: (identifier) @local.definition))

; Local variable declarations
(variable_declarator
  name: (identifier) @local.definition)

; Enhanced for loop variable
(enhanced_for_statement
  name: (identifier) @local.definition)

; Catch clause parameter
(catch_formal_parameter
  name: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
