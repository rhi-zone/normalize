; VHDL locals.scm
; architecture_body and process_statement create scopes.
; Signals are declared in architecture declarative_part;
; variables are declared in process declarative_part.
; Identifiers in declarations use `identifier` nodes;
; references use `simple_name` nodes (distinct node kind).

; Scopes
; ------

[
  (architecture_body)
  (process_statement)
] @local.scope

; Definitions
; -----------

; Signal declarations: signal x : std_logic;
(signal_declaration
  (identifier_list
    (identifier) @local.definition))

; Variable declarations in process: variable y : integer;
(variable_declaration
  (identifier_list
    (identifier) @local.definition))

; References
; ----------

; References use simple_name (e.g. in assignments, sensitivity lists)
(simple_name) @local.reference
