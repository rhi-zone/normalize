; Scopes
; ------

[
  (function_definition)
  (for_statement)
  (let_statement)
  (do_clause)
] @local.scope

; Definitions
; -----------

; Function name (first identifier child of call_expression in signature)
(function_definition
  (signature
    (call_expression
      . (identifier) @local.definition)))

; Function parameters (identifiers inside argument_list of the signature)
(function_definition
  (signature
    (call_expression
      (argument_list
        (identifier) @local.definition))))

; For loop variable (first identifier in for_binding)
(for_statement
  (for_binding
    (identifier) @local.definition))

; Let bindings (left-hand side of assignment in let_statement)
(let_statement
  (assignment
    . (identifier) @local.definition))

; References
; ----------

(identifier) @local.reference
