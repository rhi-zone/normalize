; Scopes
; ------

[
  (method)
  (singleton_method)
  (lambda)
  (block)
  (do_block)
] @local.scope

; Definitions
; -----------

; Method names
(method
  name: (identifier) @local.definition)

(singleton_method
  name: (identifier) @local.definition)

; Method parameters
(method_parameters
  (identifier) @local.definition)

; Block parameters
(block_parameters
  (identifier) @local.definition)

; Lambda parameters
(lambda_parameters
  (identifier) @local.definition)

; For loop pattern (simple identifier)
(for
  pattern: (identifier) @local.definition)

; Assignments (simple identifier LHS)
(assignment
  left: (identifier) @local.definition)

; References
; ----------

(identifier) @local.reference
