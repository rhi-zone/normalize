; Rust type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, Bar
(type_identifier) @type.reference

; Scoped types: std::collections::HashMap — capture the leaf name
(scoped_type_identifier
  name: (type_identifier) @type.reference)

; Type definitions: struct, enum, type alias
(struct_item name: (type_identifier) @name) @definition.type
(enum_item name: (type_identifier) @name) @definition.type
(type_item name: (type_identifier) @name) @definition.type
