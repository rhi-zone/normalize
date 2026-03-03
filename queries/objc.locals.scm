; Source: workspace, based on c.locals.scm extended with Objective-C method syntax.
; Arborium had only "; inherits: c" (Neovim-specific directive).

; Scopes
[
  (compound_statement)
  (function_definition)
  (method_definition)
  (for_statement)
  (if_statement)
  (while_statement)
  (do_statement)
  (switch_statement)
] @local.scope

; Definitions

; C-style function names
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @local.definition))

; C parameters
(parameter_declaration
  declarator: (identifier) @local.definition)

(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @local.definition))

; Local variable declarations
(declaration
  declarator: (identifier) @local.definition)

(init_declarator
  declarator: (identifier) @local.definition)

; ObjC method parameter (the local variable name after the type, no named field)
(method_parameter
  (identifier) @local.definition)

; References
(identifier) @local.reference
