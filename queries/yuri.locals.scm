; Yuri locals.scm
; Yuri is a domain-specific shader language (WIP).
; function_item holds name + function_parameters + block body.
; variable_item is a let-binding inside a block.
; symbol is the leaf kind for names; identifier wraps symbol in reference position.

; Scopes
; ------

; Function item creates a scope encompassing parameters and body
(function_item) @local.scope

; Definitions
; -----------

; Function name: first named child of function_item (before function_parameters)
(function_item . (symbol) @local.definition)

; Function parameters: symbol inside parameter
(parameter (symbol) @local.definition)

; Let bindings: symbol inside variable_item
(variable_item (symbol) @local.definition)

; References
; ----------

; Variable references: symbol inside identifier expression
(identifier (symbol) @local.reference)
