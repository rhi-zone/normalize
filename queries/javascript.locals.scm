; Scopes
; ------

[
  (statement_block)
  (function_declaration)
  (function_expression)
  (arrow_function)
  (method_definition)
  (class_declaration)
] @local.scope

; Definitions
; -----------

; Function names
(function_declaration
  name: (identifier) @local.definition)

(function_expression
  name: (identifier) @local.definition)

; Variable declarations
(variable_declarator
  name: (identifier) @local.definition)

; Function parameters — simple identifier
(formal_parameters
  (identifier) @local.definition)

; Object destructuring parameter: function f({ a, b }) {}
(formal_parameters
  (object_pattern
    (shorthand_property_identifier_pattern) @local.definition))

; Array destructuring parameter: function f([x, y]) {}
(formal_parameters
  (array_pattern
    (identifier) @local.definition))

; Default parameter: function f(c = 1) {}
(formal_parameters
  (assignment_pattern
    left: (identifier) @local.definition))

; Arrow function single parameter (no parentheses)
(arrow_function
  parameter: (identifier) @local.definition)

; Catch clause parameter
(catch_clause
  parameter: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
