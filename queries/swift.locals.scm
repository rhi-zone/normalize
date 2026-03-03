; Source: arborium (tree-sitter-swift), extended with parameter definitions and
; @local.reference (arborium had scopes + function names only).

(import_declaration
  (identifier) @local.definition.import)

(function_declaration
  name: (simple_identifier) @local.definition.function)

; Function parameters (internal name used inside the body)
(parameter
  name: (simple_identifier) @local.definition)

; Scopes
[
  (statements)
  (for_statement)
  (while_statement)
  (repeat_while_statement)
  (do_statement)
  (if_statement)
  (guard_statement)
  (switch_statement)
  (property_declaration)
  (function_declaration)
  (class_declaration)
  (protocol_declaration)
] @local.scope

; References
(simple_identifier) @local.reference
