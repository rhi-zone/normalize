; Python calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls

; Simple call: func()
(call
  function: (identifier) @call)

; Method call: obj.method()
(call
  function: (attribute
    object: (_) @call.qualifier
    attribute: (identifier) @call))
