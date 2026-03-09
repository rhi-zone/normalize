; Kotlin calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call_expression
  (simple_identifier) @call)

; Member/navigation call: obj.method()
(call_expression
  (navigation_expression
    (_) @call.qualifier
    (navigation_suffix
      (simple_identifier) @call)))
