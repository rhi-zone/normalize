; Typst locals.scm
; Typst code uses #let for bindings. Simple bindings: #let x = 1.
; Function shorthand: #let foo(a, b) = a + b.
; The `let` node (named) wraps both forms.

; Scopes
; ------

; The entire document is the scope (no block-level scoping in Typst content)
(source_file) @local.scope

; Definitions
; -----------

; Simple binding: #let x = 1 — ident is a direct child of let
(let
  (ident) @local.definition)

; Function name: #let foo(a, b) = ... — ident inside call
(let
  (call
    (ident) @local.definition))

; Function parameters: ident nodes in the parameter group
(let
  (call
    (group
      (ident) @local.definition)))

; References
; ----------

(ident) @local.reference
