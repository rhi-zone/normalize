; HCL (HashiCorp Configuration Language) locals.scm
; HCL is declarative with no traditional block scoping.
; Attribute names within block bodies are the primary definition sites.
; References use variable_expr (var.x, local.x) and get_attr (.name).

; Scopes
; ------

; The config file body is the global scope
(config_file) @local.scope

; Each block creates a nested scope for its attributes
(block
  (body)) @local.scope

; Definitions
; -----------

; All attribute names in any block body (locals, resource, data, module, etc.)
(attribute
  (identifier) @local.definition)

; References
; ----------

; Variable access: var.x, local.x
(variable_expr
  (identifier) @local.reference)

; Attribute access chain: .name
(get_attr
  (identifier) @local.reference)
