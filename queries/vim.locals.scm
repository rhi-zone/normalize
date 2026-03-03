; Vim script locals.scm
; function_definition creates a scope. function_declaration contains the name
; and parameters. let_statement captures variable definitions.

; Scopes
; ------

(function_definition) @local.scope

; Definitions
; -----------

; Plain function name: function Foo(...)
(function_declaration
  (identifier) @local.definition)

; Scoped function name: function s:Foo(...), function g:Foo(...)
(function_declaration
  (scoped_identifier
    (identifier) @local.definition))

; Function parameters
(parameters
  (identifier) @local.definition)

; let x = val (plain identifier on LHS)
; Dot after "let" restricts to the identifier adjacent to the keyword (not RHS)
(let_statement
  "let" .
  (identifier) @local.definition)

; let l:x = val, let s:x = val (scoped identifier on LHS)
(let_statement
  "let" .
  (scoped_identifier
    (identifier) @local.definition))

; References
; ----------

(identifier) @local.reference
