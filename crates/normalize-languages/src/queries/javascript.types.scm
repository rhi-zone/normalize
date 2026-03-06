; Type reference query for JavaScript
; JavaScript is dynamically typed; captures class names used in
; inheritance (extends) and constructor calls (new).

; Superclass in class declaration: class Foo extends Bar
(class_heritage
  (identifier) @type.reference)

; Superclass via member expression: class Foo extends ns.Bar
(class_heritage
  (member_expression
    property: (property_identifier) @type.reference))
