; Swift calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
; call_expression has no fields — children are the callee and call_suffix
(call_expression
  (simple_identifier) @call
  (call_suffix))

; Member/navigation call: obj.method()
(call_expression
  (navigation_expression
    target: (_) @call.qualifier
    suffix: (navigation_suffix
      (simple_identifier) @call))
  (call_suffix))
