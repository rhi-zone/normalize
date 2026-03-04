; GraphQL tags — definitions for types, interfaces, enums, unions, inputs, scalars, operations
; GraphQL grammar nodes have no fields — the name is a positional (name) child.

(object_type_definition
  (name) @name) @definition.class

(interface_type_definition
  (name) @name) @definition.interface

(enum_type_definition
  (name) @name) @definition.type

(union_type_definition
  (name) @name) @definition.type

(input_object_type_definition
  (name) @name) @definition.class

(scalar_type_definition
  (name) @name) @definition.type

(operation_definition
  (name) @name) @definition.function

(fragment_definition
  (fragment_name (name) @name)) @definition.function

(field_definition
  (name) @name) @definition.method
