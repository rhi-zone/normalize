; Scopes
; ------

(template_body) @local.scope
(lambda_expression) @local.scope

; function_declaration and function_definition both create scopes
(function_declaration
  name: (identifier) @local.definition) @local.scope

(function_definition
  name: (identifier) @local.definition) @local.scope

; Definitions
; -----------

(parameter
  name: (identifier) @local.definition)

(binding
  name: (identifier) @local.definition)

(val_definition
  pattern: (identifier) @local.definition)

(var_definition
  pattern: (identifier) @local.definition)

(val_declaration
  name: (identifier) @local.definition)

(var_declaration
  name: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
