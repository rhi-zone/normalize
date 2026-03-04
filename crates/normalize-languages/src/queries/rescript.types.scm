; ReScript type reference query
; Captures type identifiers used in type annotations and definitions.
;
; ReScript (formerly BuckleScript/Reason) is statically typed with ML-style types.
; `type_identifier` nodes name types in annotations and definitions.

; Simple type identifier: int, string, MyType
(type_identifier) @type.reference

; Qualified type path: Belt.Map.t
(type_identifier_path
  (type_identifier) @type.reference)
