; Go type definitions
; Captures type names from struct and interface definitions.

; Type definition: type Stack struct {...}
(type_spec
  name: (type_identifier) @name) @definition.type

; Qualified type references: io.Reader, http.Handler
(qualified_type
  package: (package_identifier) @type.qualifier
  name: (type_identifier) @name)
