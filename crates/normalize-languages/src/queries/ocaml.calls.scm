; OCaml calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for module-qualified calls
;
; OCaml uses juxtaposition for function application: `f x y` is represented
; as nested `application_expression` nodes, not a single call with argument list.
; The outermost `application_expression` is the full call.

; Function application: f x
(application_expression
  function: (value_path
    (value_name) @call))

; Module-qualified call: Module.func arg
(application_expression
  function: (value_path
    (module_path) @call.qualifier
    (value_name) @call))

; Method call: obj#method arg
(application_expression
  function: (method_invocation
    (_) @call.qualifier
    (method_name) @call))
