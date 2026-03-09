; Java type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, ArrayList
(type_identifier) @type.reference

; Scoped types: java.util.List — capture the leaf type_identifier
(scoped_type_identifier
  (type_identifier) @type.reference)

; Class definitions
(class_declaration name: (identifier) @name) @definition.type
