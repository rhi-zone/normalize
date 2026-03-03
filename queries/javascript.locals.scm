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

; Binding leaf kinds for @local.definition.each recursion.
; The engine collects these node kinds and uses them when recursing into pattern nodes.
(identifier) @local.binding-leaf
(shorthand_property_identifier_pattern) @local.binding-leaf

; Function parameters — @local.definition.each recurses into each direct child of
; formal_parameters and collects all binding leaf nodes at any nesting depth.
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
