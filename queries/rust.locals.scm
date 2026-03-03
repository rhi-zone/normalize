; Scopes
; ------

[
  (block)
  (function_item)
  (closure_expression)
  (for_expression)
  (if_expression)
  (loop_expression)
  (while_expression)
  (match_arm)
] @local.scope

; Definitions
; -----------

; Function names
(function_item
  name: (identifier) @local.definition)

; Function parameters
(parameter
  pattern: (identifier) @local.definition)

; Closure parameters
(closure_parameters
  (identifier) @local.definition)

; Let bindings (simple identifier pattern)
(let_declaration
  pattern: (identifier) @local.definition)

; For loop variables (simple identifier pattern)
(for_expression
  pattern: (identifier) @local.definition)

; Match arm bindings (simple identifier pattern)
(match_arm
  pattern: (match_pattern
    (identifier) @local.definition))

; References
; ----------

(identifier) @local.reference
