; Meson locals.scm
; Meson is a build configuration language. No user-defined functions.
; Variable assignments use operatorunit; foreach loops use foreach_command.
; References use variableunit.

; Scopes
; ------

(foreach_command) @local.scope

; Definitions
; -----------

; Variable assignment: x = value (first identifier is the LHS)
(operatorunit .
  (identifier) @local.definition)

; foreach loop variable: foreach item : list
(foreach_command
  (identifier) @local.definition)

; References
; ----------

; Variable references in expressions
(variableunit
  (identifier) @local.reference)
