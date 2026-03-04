; VHDL type reference query
; Captures type marks (type names) used in subtype indications.
;
; In VHDL, `type_mark` appears wherever a type is referenced —
; in signal declarations, port lists, variable declarations, etc.
; `subtype_indication` wraps type marks with optional constraints.

; Plain type mark: std_logic, integer, MyType
(type_mark
  (simple_name) @type.reference)

; Package-qualified type: ieee.std_logic_1164.std_logic
(type_mark
  (selected_name
    suffix: (simple_name) @type.reference))
