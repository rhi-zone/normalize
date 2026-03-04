; PowerShell type reference query
; Captures type names used in type literals and cast expressions.
;
; PowerShell uses [TypeName] syntax for type references (type literals).
; The `type_literal` wraps a `type_spec` which contains `type_name`.

; Type literal: [int], [string], [System.Collections.Generic.List[int]]
(type_literal
  (type_spec
    (type_name) @type.reference))
