; Type reference query for OCaml
; Captures type constructor references used in type expressions.

; Type constructor paths: Foo.bar, Foo.Bar.t
(type_constructor_path
  (type_constructor) @type.reference)

; Plain type constructors: t, int, string
(type_constructor) @type.reference
