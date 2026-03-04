; Elm type reference query
; Captures type constructor references used in type annotations and expressions.
;
; Elm is statically typed. Type names are uppercase identifiers.
; `type_ref` nodes (inside `type_expression`) represent concrete type references.

; Type reference: Maybe, Int, String, List a
(type_ref
  (upper_case_qid
    (upper_case_identifier) @type.reference))

; Module-qualified type: Html.Attribute, Json.Value
(type_ref
  (upper_case_qid
    (upper_case_identifier) @type.reference
    (upper_case_identifier) @type.reference))
