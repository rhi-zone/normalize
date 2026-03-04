; Swift tags query
; In the Swift grammar, class/struct/enum/actor all use class_declaration
; with declaration_kind distinguishing them. Protocol has its own protocol_declaration.

; Function declarations
(function_declaration
  name: (simple_identifier) @name) @definition.function

; Class/struct/enum/actor declarations (distinguished by declaration_kind)
(class_declaration
  name: (type_identifier) @name) @definition.class

; Protocol declarations (interfaces)
(protocol_declaration
  name: (type_identifier) @name) @definition.interface

; Type alias declarations
(typealias_declaration
  name: (type_identifier) @name) @definition.type
