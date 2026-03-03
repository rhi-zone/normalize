; Scopes
; ------

[
  (compound_statement)
  (function_definition)
  (for_statement)
  (if_statement)
  (while_statement)
  (do_statement)
  (switch_statement)
] @local.scope

; Definitions
; -----------

; Function names (via declarator chain)
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @local.definition))

; Pointer-to-function names
(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @local.definition)))

; Parameters
(parameter_declaration
  declarator: (identifier) @local.definition)

(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @local.definition))

; Local variable declarations (simple)
(declaration
  declarator: (identifier) @local.definition)

; Local variable declarations with initializer
(init_declarator
  declarator: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
