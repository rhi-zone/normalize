; MATLAB locals.scm
; function_definition has an optional function_output (return values),
; then an identifier (function name), then function_arguments (params),
; then a block body. Local variables emerge from assignment LHS.

; Scopes
; ------

(function_definition) @local.scope

; Definitions
; -----------

; Return variable(s): function y = foo(x) — y is defined in the function
(function_output
  (identifier) @local.definition)

; Function name without output: function foo(x) — identifier is first named child
(function_definition .
  (identifier) @local.definition)

; Function name with output: function y = foo(x) — identifier after function_output
(function_definition
  (function_output) .
  (identifier) @local.definition)

; Function parameters
(function_arguments
  (identifier) @local.definition)

; Local variable assignments (first identifier = LHS)
(assignment .
  (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
