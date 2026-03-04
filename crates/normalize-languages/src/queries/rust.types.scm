; Rust type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, Bar
(type_identifier) @type.reference

; Scoped types: std::collections::HashMap — capture the leaf name
(scoped_type_identifier
  name: (type_identifier) @type.reference)
