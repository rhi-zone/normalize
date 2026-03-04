; jq tags query
;
; jq function definitions: def name(params): body;
; The first identifier child of funcdef is the function name.

(funcdef
  (identifier) @name) @definition.function
