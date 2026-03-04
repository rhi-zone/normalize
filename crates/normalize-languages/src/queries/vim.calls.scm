; VimScript calls query
; @call — function call expression
; @call.qualifier — not applicable
;
; VimScript has two forms of function calls:
; 1. Expression calls: `Foo()`, `s:Foo()` — `call_expression` nodes with a
;    `function` field that is either an `identifier` or `scoped_identifier`.
; 2. Statement calls: `call Foo()` — `call_statement` wraps a `call_expression`.
;    The inner `call_expression` is matched by the rules below.

; Simple function call: Foo()
(call_expression
  function: (identifier) @call)

; Scoped function call: s:Foo(), g:Bar(), etc.
(call_expression
  function: (scoped_identifier
    (identifier) @call))
