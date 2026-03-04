; Typst tags — let bindings (function and variable definitions)

; #let name(params) = body  — function definition via call pattern
(let
  pattern: (call
    item: (ident) @name)) @definition.function

; #let name = value  — variable/constant binding
(let
  pattern: (ident) @name) @definition.var
