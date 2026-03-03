; Lean 4 locals.scm
; Lean uses def/theorem/abbrev for top-level declarations.
; Parameters live in explicit_binder/implicit_binder nodes.
; let expressions bind a name before :=.
; fun expressions have a parameters node.

; Scopes
; ------

(def) @local.scope
(fun) @local.scope

; Definitions
; -----------

; def/theorem/abbrev name: first identifier after "def" keyword
(def "def" . (identifier) @local.definition)

; Explicit binders: (n : Nat) — first named child is the variable name
(explicit_binder . (identifier) @local.definition)

; Implicit binders: {α : Type} — first named child is the variable name
(implicit_binder . (identifier) @local.definition)

; let binding: let x := 5 — first identifier after "let" keyword
(let "let" . (identifier) @local.definition)

; fun parameters: fun x => ...
(fun (parameters (identifier) @local.definition))

; References
; ----------

(identifier) @local.reference
