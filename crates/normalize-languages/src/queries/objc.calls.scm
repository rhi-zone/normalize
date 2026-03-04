; Objective-C calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; Objective-C has two call forms:
;   - C-style: func(args) — call_expression
;   - ObjC message send: [receiver selector args] — message_expression

; C-style function call: func(args)
(call_expression
  function: (identifier) @call)

; C-style with qualifier: obj->method(args)
(call_expression
  function: (field_expression
    value: (_) @call.qualifier
    field: (field_identifier) @call))

; ObjC message send: [receiver message:arg]
(message_expression
  receiver: (_) @call.qualifier
  (selector) @call)
