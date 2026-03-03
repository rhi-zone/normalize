; AWK locals.scm
; func_def creates a function scope. Parameters are in param_list.
; Variables assigned in function bodies (assignment_exp LHS) are local.
; Variables in rule blocks (BEGIN/END/{pattern}) are global.

; Scopes
; ------

(func_def) @local.scope

; Definitions
; -----------

; Function name: identifier after "function" keyword
(func_def "function" .
  (identifier) @local.definition)

; Function parameters
(func_def
  (param_list
    (identifier) @local.definition))

; Local variable assignments inside function body (LHS only)
(assignment_exp .
  (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
