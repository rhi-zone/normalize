; GraphQL tags — definitions for types, interfaces, enums, unions, inputs, scalars, operations

(object_type_definition
  name: (name) @name) @definition.class

(interface_type_definition
  name: (name) @name) @definition.interface

(enum_type_definition
  name: (name) @name) @definition.type

(union_type_definition
  name: (name) @name) @definition.type

(input_object_type_definition
  name: (name) @name) @definition.class

(scalar_type_definition
  name: (name) @name) @definition.type

(operation_definition
  name: (name) @name) @definition.function

(fragment_definition
  name: (fragment_name (name) @name)) @definition.function

(field_definition
  name: (name) @name) @definition.method
