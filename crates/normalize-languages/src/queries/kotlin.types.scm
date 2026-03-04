; Kotlin type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, String
(type_identifier) @type.reference

; User types (possibly qualified): foo.Bar — capture each component
(user_type
  (type_identifier) @type.reference)
