; Agda type reference query
; Captures type names used in type signatures and typed bindings.
;
; In Agda, type_signature nodes declare the type of a name.
; Typed bindings (used in function arguments) also carry type expressions.
; The `qid` node represents qualified identifiers used as type references.

; Type signature: name : Type
; The `expr` children of a type_signature are the type expressions.
(type_signature
  (expr) @type.reference)
