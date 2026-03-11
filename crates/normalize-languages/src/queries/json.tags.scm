; JSON key-value pairs as symbols.
; All pairs are captured as definition.var; container nesting is derived
; from the AST structure (pair > object > pair).

(pair
  key: (string (string_content) @name)) @definition.var
