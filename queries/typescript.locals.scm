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

; Function parameters — simple identifier
(required_parameter
  pattern: (identifier) @local.definition)

(optional_parameter
  pattern: (identifier) @local.definition)

; Object destructuring parameter: function f({ a, b }: T) {}
(required_parameter
  pattern: (object_pattern
    (shorthand_property_identifier_pattern) @local.definition))

(optional_parameter
  pattern: (object_pattern
    (shorthand_property_identifier_pattern) @local.definition))

; Array destructuring parameter: function f([x, y]: U) {}
(required_parameter
  pattern: (array_pattern
    (identifier) @local.definition))

(optional_parameter
  pattern: (array_pattern
    (identifier) @local.definition))

; Arrow function single parameter (no parentheses)
(arrow_function
  parameter: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
