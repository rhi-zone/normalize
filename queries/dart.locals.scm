; Dart grammar note: top-level function declarations are represented as
; (function_signature) followed by (function_body) as siblings — there is no
; wrapping function_declaration node. This means function parameters defined
; inside function_signature cannot be resolved from the sibling function_body.
; Within-block resolution (local variables, for-loop variables) works correctly.

; Scopes
; ------

[
  (function_signature)
  (function_expression)
  (block)
  (for_statement)
  (do_statement)
] @local.scope

; Definitions
; -----------

; Function names (top-level and nested)
(function_signature
  name: (identifier) @local.definition)

; Class names
(class_definition
  name: (identifier) @local.definition)

; Function parameters
; formal_parameter's name field is optional; use direct child (identifier)
; which captures the param name but not the type (type_identifier ≠ identifier)
(formal_parameter
  (identifier) @local.definition)

; Local variable declarations (var x = ..., final x = ..., etc.)
(initialized_variable_definition
  name: (identifier) @local.definition)

; For-in loop variable (for (var x in list))
(for_loop_parts
  name: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
