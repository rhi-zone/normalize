; HLSL (High-Level Shader Language) locals.scm
; HLSL is C-like with HLSL-specific semantics annotations (: SV_Position, etc.)
; which are separate `semantics` nodes and can be ignored for scoping.
; Function definitions, parameters, and local variables follow C grammar structure.

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

; Function names
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @local.definition))

; Parameters (semantics annotation is a sibling `semantics` node, ignored)
(parameter_declaration
  declarator: (identifier) @local.definition)

(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @local.definition))

; Local variable declarations (simple or with initializer)
(declaration
  declarator: (identifier) @local.definition)

(init_declarator
  declarator: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
