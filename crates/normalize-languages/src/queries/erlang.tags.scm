; Erlang tags query
; Covers: function clauses, module declarations, record declarations, type aliases

; Function clauses
(function_clause
  name: (atom) @name) @definition.function

; Module declaration: -module(name).
(module_attribute
  name: (atom) @name) @definition.module

; Record declarations: -record(name, {...}).
(record_decl
  name: (atom) @name) @definition.class

; Type aliases: -type name() :: ...
(type_alias
  name: (type_name
    name: (atom) @name)) @definition.type
