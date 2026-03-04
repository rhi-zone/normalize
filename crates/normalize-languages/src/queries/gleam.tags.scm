; Gleam tags query
; Covers: functions, type definitions, type aliases, constants

; Function definitions
(function
  name: (identifier) @name) @definition.function

; Type definitions (ADTs / custom types)
(type_definition
  name: (type_identifier) @name) @definition.class

; Type aliases
(type_alias
  name: (type_identifier) @name) @definition.type

; Constants
(constant
  name: (identifier) @name) @definition.constant
