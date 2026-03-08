; GraphQL type references
; Captures type names used in type positions: field types, variable types,
; argument types, implements clauses, and union member types.
;
; In GraphQL, `named_type` holds a type name in any type position.
; It can be wrapped in `non_null_type` (!) or `list_type` ([]).

; Named type reference: Foo, String, Boolean
(named_type
  (name) @type.reference)
