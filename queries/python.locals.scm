; Scopes
; Python has function-level scoping, not block-level.
; Only function_definition, lambda, and class_definition create new scopes.
[
  (function_definition)
  (lambda)
  (class_definition)
] @local.scope

; Definitions
; -----------

; Function names
(function_definition
  name: (identifier) @local.definition)

; Class names
(class_definition
  name: (identifier) @local.definition)

; Function parameters (simple identifier)
(parameters
  (identifier) @local.definition)

; Function parameters with defaults
(default_parameter
  name: (identifier) @local.definition)

; Typed parameters
(typed_parameter
  (identifier) @local.definition)

; Lambda parameters
(lambda_parameters
  (identifier) @local.definition)

; For loop target (function-scoped in Python)
(for_statement
  left: (identifier) @local.definition)

; Assignments (simple identifier on LHS)
(assignment
  left: (identifier) @local.definition)

; Walrus operator (:=)
(named_expression
  name: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
