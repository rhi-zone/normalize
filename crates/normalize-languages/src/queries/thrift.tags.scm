; Thrift tags query
; @name            — the symbol name
; @definition.*    — the definition node

; Struct definitions
(struct_definition
  "struct" (identifier) @name) @definition.class

; Union definitions
(union_definition
  "union" (identifier) @name) @definition.class

; Exception definitions
(exception_definition
  "exception" (identifier) @name) @definition.class

; Enum definitions
(enum_definition
  "enum" (identifier) @name) @definition.class

; Service definitions (interface-like containers)
(service_definition
  "service" (identifier) @name) @definition.interface

; Function definitions (methods inside services)
(function_definition
  (identifier) @name) @definition.function

; Typedef definitions
(typedef_definition
  (typedef_identifier) @name) @definition.type

; Constant definitions
(const_definition
  (identifier) @name) @definition.constant
