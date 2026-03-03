; Scopes
; ------

[
  (function_declaration)
  (anonymous_function)
  (lambda_literal)
  (class_declaration)
  (object_declaration)
] @local.scope

; Definitions
; -----------

; Function names: anchor to first simple_identifier (the name, before parameters)
(function_declaration
  . (simple_identifier) @local.definition)

; Function parameters (no anchor: parameter_modifiers may precede the name)
(parameter
  (simple_identifier) @local.definition)

; Optional-type parameters (fun foo(x: Int = 0))
(parameter_with_optional_type
  (simple_identifier) @local.definition)

; Direct capture for function value parameters
; (covers grammars where parameter nodes don't wrap individual params)
(function_value_parameters
  (simple_identifier) @local.definition)

; Variable/property declarations
(variable_declaration
  (simple_identifier) @local.definition)

; Lambda parameters (variable_declaration inside lambda_parameters)
(lambda_parameters
  (variable_declaration
    (simple_identifier) @local.definition))

; For loop variable
(for_statement
  (variable_declaration
    (simple_identifier) @local.definition))

; References
; ----------

(simple_identifier) @local.reference
