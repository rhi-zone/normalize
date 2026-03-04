; Idris type reference query
; Captures type names used in type signatures.
;
; Idris is a dependently-typed language. Type signatures are declared with
; `type_signature` nodes. Type expressions contain `exp_name` nodes with
; `caname` (uppercase = type/constructor) or `loname` (lowercase = type variable).

; Type name in a type signature: uppercase constructor = concrete type
(type_signature
  (exp_name
    (caname) @type.reference))

; Qualified type name in a signature: Module.Type
(type_signature
  (exp_name
    (qualified_caname) @type.reference))

; Type in type_parens (explicit type annotation)
(type_parens
  (exp_name
    (caname) @type.reference))

; Type in type_braces (implicit type annotation)
(type_braces
  (exp_name
    (caname) @type.reference))
