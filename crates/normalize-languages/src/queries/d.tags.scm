; D tags query
; D grammar nodes have no fields — names are positional identifier children

(class_declaration
  (identifier) @name) @definition.class

(struct_declaration
  (identifier) @name) @definition.class

(interface_declaration
  (identifier) @name) @definition.interface

(enum_declaration
  (identifier) @name) @definition.type
