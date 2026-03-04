; Verilog/SystemVerilog type reference query
; Captures data type references used in variable declarations, ports, and parameters.
;
; Verilog has built-in types (logic, bit, reg, wire, integer, etc.) and
; user-defined types via typedef. `data_type` covers most type references.

; Integer vector types: logic, bit, reg
(integer_vector_type) @type.reference

; Integer atom types: byte, shortint, int, longint, integer, time
(integer_atom_type) @type.reference

; Non-integer types: shortreal, real, realtime
(non_integer_type) @type.reference

; User-defined type or class reference inside data_type
(data_type
  (simple_identifier) @type.reference)

; Type reference (used in cast expressions, etc.)
(type_reference
  (data_type
    (simple_identifier) @type.reference))
