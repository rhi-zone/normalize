; Ada tags query

(package_declaration
  name: (identifier) @name) @definition.module

(package_body
  name: (identifier) @name) @definition.module

(generic_package_declaration
  name: (identifier) @name) @definition.module

(subprogram_declaration
  (subprogram_specification
    name: (identifier) @name)) @definition.function

(subprogram_body
  (subprogram_specification
    name: (identifier) @name)) @definition.function

(expression_function_declaration
  (subprogram_specification
    name: (identifier) @name)) @definition.function

(full_type_declaration
  name: (identifier) @name) @definition.type

(private_type_declaration
  name: (identifier) @name) @definition.type

(incomplete_type_declaration
  name: (identifier) @name) @definition.type
