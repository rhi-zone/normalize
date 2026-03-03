; Zsh locals.scm
; The zsh tree-sitter grammar is derived from bash but with reduced fidelity.
; function definitions, local/typeset declarations, and for loops produce
; ERROR nodes — those constructs cannot be queried.
; Only bare variable assignments (x=1) are reliably modeled.
; References via $var use simple_expansion which has no variable_name child,
; so cross-reference resolution is not available in this grammar.

; Scopes
; ------

[
  (subshell)
  (compound_command)
] @local.scope

; Definitions
; -----------

; Bare variable assignment: x=1
; Filter out shell keywords misidentified as variable names by the grammar.
((variable_assignment
  (word) @local.definition)
 (#not-match? @local.definition "^(local|typeset|declare|export|unset|readonly)$"))
