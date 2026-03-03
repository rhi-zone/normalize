; Scopes
; ------

[
  (func_declaration)
  (block_statement)
] @local.scope

; Definitions
; -----------

; Function name (first identifier child of func_declarator)
(func_declarator
  . (identifier) @local.definition)

; Function parameters and typed local variables (both use var_declarator)
(var_declarator
  (identifier) @local.definition)

; Auto declarations: auto v = expr
(auto_assignment
  . (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
