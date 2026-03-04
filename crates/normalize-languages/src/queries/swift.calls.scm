; Swift calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call_expression
  function: (simple_identifier) @call)

; Member/navigation call: obj.method()
(call_expression
  function: (navigation_expression
    target: (_) @call.qualifier
    suffix: (navigation_suffix
      (simple_identifier) @call)))
