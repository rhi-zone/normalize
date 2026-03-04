; Starlark calls query
; @call — call expression
; @call.qualifier — receiver object for method calls
;
; Starlark (Bazel build language) has Python-like syntax. Function calls are
; `call` nodes with a `function` field that holds a `primary_expression`.
; The primary expression is either an `identifier` (simple call) or an
; `attribute` (method call like `obj.method()`).

; Simple call: func(args...)
(call
  function: (primary_expression
    (identifier) @call))

; Method call: obj.method(args...)
(call
  function: (primary_expression
    (attribute
      object: (_) @call.qualifier
      attribute: (identifier) @call)))
