; Meson tags query
;
; Meson does not have first-class function definitions. Variables are assigned
; via var_unit (simple assignment) or variableunit (augmented assignment).
; Capture top-level variable assignments as definitions.

(var_unit
  value: (identifier) @name) @definition.var

(variableunit
  value: (identifier) @name) @definition.var
