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

; Function parameters
(required_parameter
  pattern: (identifier) @local.definition)

(optional_parameter
  pattern: (identifier) @local.definition)

; Arrow function single parameter (no parentheses)
(arrow_function
  parameter: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
