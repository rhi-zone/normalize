; Haskell calls query
; @call — call expression nodes
; @call.qualifier — qualifier for qualified calls
;
; Haskell uses juxtaposition for function application: `f x y` is an `apply`
; expression. The function being applied is the first child. There are no
; explicit call parentheses required.

; Function application: f x y
; The `function` field names the applied function (an expression)
(apply
  function: (variable) @call)

; Qualified call: Module.func args
(apply
  function: (qualified_variable
    (module) @call.qualifier
    (variable) @call))

; Constructor application: Foo x
(apply
  function: (constructor) @call)

; Qualified constructor: Module.Ctor x
(apply
  function: (qualified_constructor
    (module) @call.qualifier
    (constructor) @call))
