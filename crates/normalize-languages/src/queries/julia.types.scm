; Type reference query for Julia
; Captures type identifiers used in :: annotations and parametric types.

; Type annotations: x::Int, foo(x::Float64)::String
(typed_expression
  . _ @_value
  (identifier) @type.reference)

; Parametrized types: Vector{Int}, Dict{String, Any}
(parametrized_type_expression
  (identifier) @type.reference)
