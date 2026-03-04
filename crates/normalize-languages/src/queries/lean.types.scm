; Lean 4 type reference query
; Captures type identifiers used in type ascriptions and explicit type positions.
;
; Lean 4 is dependently typed — types are first-class values.
; `type_ascription` nodes carry explicit type annotations (expr : Type).

; Type ascription: (expr : Type)
(type_ascription
  type: (identifier) @type.reference)

; Subtype expression used as a type constraint
(subtype
  (identifier) @type.reference)
