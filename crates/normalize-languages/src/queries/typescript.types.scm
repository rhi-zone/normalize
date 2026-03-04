; TypeScript type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, Bar
(type_identifier) @type.reference

; Predefined types: string, number, boolean, void, any, never, object, symbol, bigint
(predefined_type) @type.reference

; Nested types: Foo.Bar — capture the full path
(nested_type_identifier) @type.reference
