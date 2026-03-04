; Java type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, ArrayList
(type_identifier) @type.reference

; Scoped types: java.util.List — capture the leaf name
(scoped_type_identifier
  name: (type_identifier) @type.reference)
