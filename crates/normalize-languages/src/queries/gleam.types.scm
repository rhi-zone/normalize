; Type reference query for Gleam
; Captures type identifiers used in function signatures and type annotations.

; Plain type identifiers: Int, String, MyType
(type_identifier) @type.reference

; Remote type identifiers: module.Type
(remote_type_identifier
  (type_identifier) @type.reference)
