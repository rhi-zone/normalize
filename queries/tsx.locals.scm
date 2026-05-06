; Source: workspace (based on typescript.locals.scm; arborium had only 2 lines).
; TSX extends TypeScript — same node types for variables, functions, parameters.

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

; Class names
(class_declaration
  name: (type_identifier) @local.definition)

; Method names
(method_definition
  name: (property_identifier) @local.definition)

; Variable declarations
(variable_declarator
  name: (identifier) @local.definition)

; Binding leaf kinds for @local.definition.each recursion.
(identifier) @local.binding-leaf
(shorthand_property_identifier_pattern) @local.binding-leaf

; Simple identifier parameters — captured as .parameter for the dead-parameter rule.
(required_parameter
  pattern: (identifier) @local.definition.parameter)

(optional_parameter
  pattern: (identifier) @local.definition.parameter)

; Destructured parameters — recurse into pattern leaves (no subkind).
(required_parameter
  pattern: (object_pattern) @local.definition.each)
(required_parameter
  pattern: (array_pattern) @local.definition.each)
(required_parameter
  pattern: (rest_pattern) @local.definition.each)

(optional_parameter
  pattern: (object_pattern) @local.definition.each)
(optional_parameter
  pattern: (array_pattern) @local.definition.each)
(optional_parameter
  pattern: (rest_pattern) @local.definition.each)

; Arrow function single parameter (no parentheses)
(arrow_function
  parameter: (identifier) @local.definition.parameter)

; References
; ----------

(identifier) @local.reference
