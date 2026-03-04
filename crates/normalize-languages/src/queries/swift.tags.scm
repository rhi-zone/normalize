; Swift tags query
; Covers: functions, classes, structs, enums, protocols, actors, extensions, type aliases

; Function declarations
(function_declaration
  name: (simple_identifier) @name) @definition.function

; Initializer declarations (constructors)
(init_declaration
  "init" @name) @definition.function

; Class declarations
(class_declaration
  name: (type_identifier) @name) @definition.class

; Struct declarations
(struct_declaration
  name: (type_identifier) @name) @definition.class

; Enum declarations
(enum_declaration
  name: (type_identifier) @name) @definition.class

; Actor declarations
(actor_declaration
  name: (type_identifier) @name) @definition.class

; Protocol declarations (interfaces)
(protocol_declaration
  name: (type_identifier) @name) @definition.interface

; Extension declarations (reference, not definition)
(extension_declaration
  name: (type_identifier) @name) @reference.implementation

; Type alias declarations
(typealias_declaration
  name: (type_identifier) @name) @definition.type
