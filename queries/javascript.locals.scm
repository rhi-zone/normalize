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

; Function parameters — simple identifier or any destructuring pattern
; @local.definition.each recurses into the node to collect all binding identifiers,
; handling arbitrary nesting depth (e.g. { a: { b } }, [[x, y]]).
(formal_parameters
  (_) @local.definition.each)

; Arrow function single parameter (no parentheses)
(arrow_function
  parameter: (identifier) @local.definition)

; Catch clause parameter
(catch_clause
  parameter: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
