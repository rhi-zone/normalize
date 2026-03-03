; PowerShell locals.scm
; function_statement creates a scope; parameters use the script_parameter node.
; Variable references and assignments use the `variable` leaf node kind.

; Scopes
; ------

(function_statement) @local.scope

(script_block) @local.scope

; Definitions
; -----------

; Function name
(function_statement
  (function_name) @local.definition)

; Function parameters (inside param block)
(script_parameter
  (variable) @local.definition)

; foreach loop variable
(foreach_statement
  (variable) @local.definition)

; References
; ----------

(variable) @local.reference
