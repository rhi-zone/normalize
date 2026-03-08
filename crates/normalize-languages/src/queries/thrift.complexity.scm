; Thrift IDL complexity query
; @complexity — nodes that increase cyclomatic complexity
; @nesting — nodes that increase nesting depth
;
; Thrift is an Interface Definition Language (IDL) — it defines service
; interfaces, data types, and constants. It contains no control flow
; (no if/for/while/match), so there are no @complexity nodes.
;
; Service and function definitions are structural containers that establish
; nesting depth for the purpose of measuring definition complexity.

; Nesting nodes
(service_definition) @nesting
(function_definition) @nesting
