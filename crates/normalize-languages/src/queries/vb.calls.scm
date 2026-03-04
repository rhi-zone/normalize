; Visual Basic calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; VB function/method calls are `invocation` nodes with a `target` field that is
; either an `identifier` (simple call) or a `member_access` (method call with
; receiver). The `member_access` node has `object` and `member` fields.

; Simple call: Func(args...)
(invocation
  target: (identifier) @call)

; Method call: obj.Method(args...)
(invocation
  target: (member_access
    object: (_) @call.qualifier
    member: (identifier) @call))
