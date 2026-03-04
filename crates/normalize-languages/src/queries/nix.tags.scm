; Nix tags query
; Covers: attribute bindings in attrsets, let expressions, and rec attrsets
; Nix's main declaration form is: name = value; inside attrsets/let
; The binding node has: attrpath (with attr: identifier) and expression fields.

; Attribute bindings: name = value;
; The attrpath's first attr is the binding name.
; e.g., in let x = 1; in ... or rec { foo = ...; }
(binding
  attrpath: (attrpath
    attr: (identifier) @name)) @definition.var

; Function-valued bindings (heuristic: binding where expression is a function)
; e.g., mkDerivation = { ... }: ...
(binding
  attrpath: (attrpath
    attr: (identifier) @name)
  expression: (function_expression)) @definition.function
