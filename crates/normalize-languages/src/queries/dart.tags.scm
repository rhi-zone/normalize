; Dart tags query
; Covers: functions, methods, classes, enums, mixins, extensions

; Class definitions
(class_definition
  name: (identifier) @name) @definition.class

; Enum declarations
(enum_declaration
  name: (identifier) @name) @definition.class

; Mixin declarations (interface-like) — name is a positional identifier child, not a field
(mixin_declaration
  (identifier) @name) @definition.interface

; Extension declarations (reference)
(extension_declaration
  name: (identifier) @name) @reference.implementation

; Top-level function signatures
(function_signature
  name: (identifier) @name) @definition.function

; Method signatures inside classes are wrapped by method_signature but actual kinds are:
; function_signature, getter_signature, setter_signature inside class_body

; Getter signatures
(getter_signature
  name: (identifier) @name) @definition.method

; Setter signatures
(setter_signature
  name: (identifier) @name) @definition.method
