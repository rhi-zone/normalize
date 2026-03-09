; Python type references (PEP 484 annotations)
; Captures identifiers used in type annotation positions.

; Type annotations on parameters and variables: x: Foo
(type
  (identifier) @type.reference)

; Dotted type annotations: x: foo.Bar
(type
  (attribute
    object: (identifier) @type.reference
    attribute: (identifier) @type.reference))

; Class definitions
(class_definition name: (identifier) @name) @definition.type
