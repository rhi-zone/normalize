; D tags query

(module_declaration
  name: (qualified_identifier) @name) @definition.module

(class_declaration
  name: (identifier) @name) @definition.class

(struct_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(auto_declaration
  (auto_declaration_part
    identifier: (identifier) @name)) @definition.function

(enum_declaration
  name: (identifier) @name) @definition.type

(alias_declaration
  (identifier) @name) @definition.type
