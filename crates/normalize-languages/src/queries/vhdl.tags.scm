; VHDL tags query

; Entity declarations
(entity_declaration
  name: (identifier) @name) @definition.class

; Architecture bodies
(architecture_body
  name: (identifier) @name) @definition.class

; Package declarations
(package_declaration
  name: (identifier) @name) @definition.module

; Full type declarations
(full_type_declaration
  name: (identifier) @name) @definition.type

; Function bodies (use designator field)
(function_body
  designator: (identifier) @name) @definition.function

; Procedure bodies (use designator field)
(procedure_body
  designator: (identifier) @name) @definition.function
