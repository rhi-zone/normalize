; Source: workspace. Arborium had 7 lines using old @scope/@definition convention.
; ReScript uses value_identifier (not identifier) for variable names.

; Scopes
[
  (let_binding)
  (switch_expression)
  (function)
  (block)
] @local.scope

; Definitions

; Let bindings: let x = ... and let f = (x) => ...
(let_binding
  pattern: (value_identifier) @local.definition)

; Function parameters
(parameter
  (value_identifier) @local.definition)

(labeled_parameter
  (value_identifier) @local.definition)

(function
  parameter: (value_identifier) @local.definition)

; References
(value_identifier) @local.reference
