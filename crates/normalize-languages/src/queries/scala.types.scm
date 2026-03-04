; Type reference query for Scala
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, String
(type_identifier) @type.reference

; Stable type identifiers (qualified): foo.Bar — capture the leaf name
(stable_type_identifier
  (type_identifier) @type.reference)
