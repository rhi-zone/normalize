; Scala tags query
; Covers: functions, methods, classes, objects, traits, type definitions

; Function definitions (top-level and inside containers)
(function_definition
  name: (identifier) @name) @definition.function

; Class definitions
(class_definition
  name: (identifier) @name) @definition.class

; Object definitions (singleton objects = modules)
(object_definition
  name: (identifier) @name) @definition.module

; Trait definitions (interfaces)
(trait_definition
  name: (identifier) @name) @definition.interface

; Type definitions (type aliases)
(type_definition
  name: (type_identifier) @name) @definition.type
