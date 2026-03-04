; VHDL calls query
; @call — function/procedure call nodes
; @call.qualifier — qualifier for selected (package-qualified) calls
;
; VHDL has two call forms:
;   - function_call: used in expressions, has a `function` field
;   - procedure_call_statement: used as statements, has a `procedure` field

; Function call: some_func(args)
(function_call
  function: (simple_name) @call)

; Package-qualified function call: pkg.func(args)
(function_call
  function: (selected_name
    prefix: (_) @call.qualifier
    suffix: (simple_name) @call))

; Procedure call: some_proc(args);
(procedure_call_statement
  procedure: (simple_name) @call)

; Package-qualified procedure call: pkg.proc(args);
(procedure_call_statement
  procedure: (selected_name
    prefix: (_) @call.qualifier
    suffix: (simple_name) @call))
