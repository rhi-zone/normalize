; Idris calls query
; @call — function application nodes
; @call.qualifier — namespace qualifier for qualified calls
;
; Idris uses juxtaposition for function application, like Haskell.
; The grammar represents expressions as `exp_name` nodes for named references
; (both functions being applied and plain references). There is no distinct
; application node — `function` declarations contain `rhs` with expressions.
; Capture `exp_name` children that are simple names (loname = lowercase, caname = uppercase).

; Simple name reference / call: foo x y
(exp_name
  (loname) @call)

; Qualified reference / call: Module.foo x y
(exp_name
  (qualified_loname) @call)

; Constructor call: Foo x
(exp_name
  (caname) @call)

; Qualified constructor: Module.Foo x
(exp_name
  (qualified_caname) @call)
