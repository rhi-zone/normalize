; Go type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, Bar
(type_identifier) @type.reference

; Qualified types: io.Reader, http.Handler — capture both parts
(qualified_type
  package: (package_identifier) @type.reference
  name: (type_identifier) @type.reference)
