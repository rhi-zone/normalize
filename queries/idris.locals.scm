; Idris 2 locals.scm
; Idris uses function/lhs/funvar for all top-level definitions.
; Pattern arguments appear in funvar > patterns > pat_name.
; References use exp_name > loname (not bare loname).
; Lowercase names: loname; uppercase/constructor names: caname.

; Scopes
; ------

; Each function clause creates a scope for its pattern variables
(function) @local.scope

; Definitions
; -----------

; Function name: first loname in funvar (the applied function name)
(funvar . (loname) @local.definition)

; Pattern parameters in function clauses: add x y = ...
(pat_name (loname) @local.definition)

; References
; ----------

; Variable references in expressions
(exp_name (loname) @local.reference)
