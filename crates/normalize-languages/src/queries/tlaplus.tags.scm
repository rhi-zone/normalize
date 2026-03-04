; TLA+ tags query
; Covers: module definitions and operator definitions
; TLA+ files contain a top-level module with a name and operator definitions inside.

; Module definition: ---- MODULE ModuleName ----
(module
  name: (identifier) @name) @definition.module

; Operator definition: OpName(args) == expr
; The operator_definition has a name field which can be identifier or operator symbol.
(operator_definition
  name: (identifier) @name) @definition.function
