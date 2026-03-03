; Scopes
; ------

[
  (function_definition)
  (closure)
] @local.scope

; Definitions
; -----------

; Function name (direct identifier child of function_definition)
(function_definition
  (identifier) @local.definition)

; Function and closure parameters
(parameter
  (identifier) @local.definition)

; Variable declarations: def v = ...
; Use anchor to capture only the name, not identifier values
(declaration
  . (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
