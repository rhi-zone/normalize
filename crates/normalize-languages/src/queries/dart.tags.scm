; Dart tags query
; Covers: functions, methods, classes, enums, mixins, extensions, type aliases

; Class definitions
(class_definition
  name: (identifier) @name) @definition.class

; Enum declarations
(enum_declaration
  name: (identifier) @name) @definition.class

; Mixin declarations (interface-like)
(mixin_declaration
  name: (identifier) @name) @definition.interface

; Extension declarations (reference)
(extension_declaration
  name: (identifier) @name) @reference.implementation

; Type aliases
(type_alias
  name: (identifier) @name) @definition.type

; Top-level function signatures
(function_signature
  name: (identifier) @name) @definition.function

; Method signatures (inside classes)
(method_signature
  name: (identifier) @name) @definition.method

; Getter signatures
(getter_signature
  name: (identifier) @name) @definition.method

; Setter signatures
(setter_signature
  name: (identifier) @name) @definition.method
