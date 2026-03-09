; Idris type reference query
; Captures type names used in type signatures.

; Type names (uppercase) in signatures: distance : Point -> Point -> Double
(signature
  (exp_name
    (caname) @type.reference))

; Qualified type names: Module.Type
(signature
  (exp_name
    (qualified_caname) @type.reference))

; Type in parenthesized context
(type_parens
  (exp_name
    (caname) @type.reference))

; Type in braces (implicit)
(type_braces
  (exp_name
    (caname) @type.reference))
