; VB.NET locals.scm
; Visual Basic has methods/functions/subs, parameters, Dim declarations,
; and For Each loop variables. Uses identifier nodes with named fields.

; Scopes
; ------

(method_declaration) @local.scope
(lambda_expression) @local.scope

; Definitions
; -----------

; Function/Sub name: first identifier in method_declaration
(method_declaration name: (identifier) @local.definition)

; Function/Sub parameters
(parameter name: (identifier) @local.definition)

; Local variable declarations: Dim x As Integer
(dim_statement name: (identifier) @local.definition)

; For Each loop variable: For Each item In items
(for_each_statement variable: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
