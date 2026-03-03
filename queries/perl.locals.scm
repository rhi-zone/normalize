; Scopes
; ------

[
  (block)
] @local.scope

; Definitions
; -----------

; Subroutine name
(subroutine_declaration_statement
  (bareword) @local.definition)

; my/our/local scalar variables ($var)
(variable_declaration
  (scalar
    (varname) @local.definition))

; my/our/local array variables (@arr)
(variable_declaration
  (array
    (varname) @local.definition))

; my/our/local hash variables (%hash)
(variable_declaration
  (hash
    (varname) @local.definition))

; References
; ----------

; Variable references ($var, @arr, %hash) — captures the name without sigil
(varname) @local.reference
