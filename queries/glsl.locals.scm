; GLSL (OpenGL Shading Language) locals.scm
; GLSL is C-like. Function definitions, parameter declarations, and local
; variable declarations use the same AST structure as C.
; uniform/in/out globals are captured at translation_unit level.

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

; Parameters
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
