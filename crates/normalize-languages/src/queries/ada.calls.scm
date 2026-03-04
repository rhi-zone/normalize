; Ada calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; Ada has two call forms:
;   - function_call: used in expressions
;   - procedure_call_statement: used as statements
; Both have a `name` field for the callee.

; Function call: Foo(args)
(function_call
  name: (identifier) @call)

; Function call with qualified name: Package.Foo(args)
(function_call
  name: (selected_component
    prefix: (_) @call.qualifier
    selector_name: (identifier) @call))

; Procedure call statement: Foo(args); or Foo;
(procedure_call_statement
  name: (identifier) @call)

; Procedure call with qualified name: Package.Foo(args);
(procedure_call_statement
  name: (selected_component
    prefix: (_) @call.qualifier
    selector_name: (identifier) @call))
