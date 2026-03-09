; ReScript tags query

(let_binding
  pattern: (value_identifier) @name) @definition.function

(external_declaration
  (value_identifier) @name) @definition.function

(type_declaration
  (type_binding
    name: (type_identifier) @name)) @definition.type

(module_declaration
  (module_binding
    name: (module_identifier) @name)) @definition.module
