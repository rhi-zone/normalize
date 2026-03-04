; Elm calls query
; @call — function application nodes
; @call.qualifier — module qualifier for qualified calls
;
; Elm is a purely functional language using juxtaposition for application.
; `f x y` is a `function_call_expr` with a `target` field for the function
; and `arg` fields for arguments.

; Simple function application: f x
(function_call_expr
  target: (value_expr
    name: (value_qid
      (lower_case_identifier) @call)))

; Module-qualified call: Module.func x
(function_call_expr
  target: (value_expr
    name: (value_qid
      (upper_case_identifier) @call.qualifier
      (lower_case_identifier) @call)))

; Constructor application: Foo x (uppercase constructors)
(function_call_expr
  target: (value_expr
    name: (upper_case_qid
      (upper_case_identifier) @call)))
