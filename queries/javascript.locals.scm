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

; Simple identifier parameters — captured as .parameter so the dead-parameter rule works.
(formal_parameters
  (identifier) @local.definition.parameter)

; Default-valued parameters where the binding name is a simple identifier.
(formal_parameters
  (assignment_pattern
    left: (identifier) @local.definition.parameter))

; Destructured and rest parameters — recurse into pattern leaves (no subkind since
; the recursion can't propagate one; these won't be reported as dead).
(formal_parameters
  (object_pattern) @local.definition.each)
(formal_parameters
  (array_pattern) @local.definition.each)
(formal_parameters
  (rest_pattern) @local.definition.each)

; Arrow function single parameter (no parentheses)
(arrow_function
  parameter: (identifier) @local.definition.parameter)

; Catch clause parameter
(catch_clause
  parameter: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
