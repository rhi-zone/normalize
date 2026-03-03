; jq locals.scm
; jq has function definitions (def foo(x): ...) and as-binding patterns
; (. as $item | ...). Both create scoped names.
; Variables use the `variable` node (e.g. $item).

; Scopes
; ------

(funcdef) @local.scope

; Definitions
; -----------

; Function name: identifier after "def" keyword
(funcdef "def" .
  (identifier) @local.definition)

; Function parameters: identifiers in funcdefargs
(funcdefargs
  (identifier) @local.definition)

; as-binding pattern: . as $item
(binding
  (variable) @local.definition)

; References
; ----------

(variable) @local.reference
