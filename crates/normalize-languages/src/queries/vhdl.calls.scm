; VHDL calls query
; @call — function/procedure call nodes
; @call.qualifier — qualifier for selected (package-qualified) calls
;
; VHDL has two call forms:
;   - ambiguous_name: used in expressions (parser can't distinguish function
;     calls from array indexing without type info, so uses ambiguous_name)
;   - function_call: used when parser can resolve it unambiguously
;   - procedure_call_statement: used as statements, has a `procedure` field

; Function call in expression context (ambiguous with array indexing)
(ambiguous_name
  prefix: (simple_name) @call)

; Package-qualified ambiguous call: pkg.func(args)
(ambiguous_name
  prefix: (selected_name
    prefix: (_) @call.qualifier
    suffix: (simple_name) @call))

; Explicit function_call form (unambiguous)
(function_call
  function: (simple_name) @call)

; Package-qualified explicit function call: pkg.func(args)
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
