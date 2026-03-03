; Agda locals.scm
; Agda uses homoiconic function nodes for both type signatures and equations.
; Type signatures have a function_name wrapper in lhs; equation clauses do not.
; Note: let-binding patterns cause query Structure errors (grammar node-type
; restriction: lhs > atom is invalid inside let-bound functions at compile time).
; Note: where clauses appear as sibling functions at source_file level (grammar
; does not nest where blocks inside the parent function).

; Scopes
; ------

; The whole file is the module scope for top-level function names
(source_file) @local.scope

; Definitions
; -----------

; Type signature form: `name : Type` — only the sig clause has function_name
(function
  (lhs
    (function_name
      (atom (qid) @local.definition))))

; References
; ----------

; All qualified identifiers used in expressions
(atom (qid) @local.reference)
