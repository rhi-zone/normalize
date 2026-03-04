; C++ type references
; Captures type identifiers used in type positions.

; Plain type identifiers: Foo, MyClass
(type_identifier) @type.reference

; Qualified identifiers: std::vector, ns::Type — capture the name component
(qualified_identifier
  name: (type_identifier) @type.reference)

; Template types: vector<int> — capture the template name
(template_type
  name: (type_identifier) @type.reference)
