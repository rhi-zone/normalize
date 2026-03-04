; F# calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; F# uses `application_expression` for function application (juxtaposition):
; `f x y` is an application_expression. Method calls use dot access.

; Function application: f x y
(application_expression
  function: (long_identifier
    (identifier) @call))

; Qualified call: Module.func args
(application_expression
  function: (long_identifier
    (_) @call.qualifier
    (identifier) @call))

; Method call: obj.Method(args)
(dot_expression
  base: (_) @call.qualifier
  field: (long_identifier
    (identifier) @call))
