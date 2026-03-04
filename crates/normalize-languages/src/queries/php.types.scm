; PHP type reference query
; Captures type names used in type annotations (PHP 7+ typed properties,
; parameter types, return types, and union types).

; Named type: Foo, Bar, int (in type position)
(named_type
  (name) @type.reference)

; Qualified name as type: \Foo\Bar
(named_type
  (qualified_name) @type.reference)

; Primitive type: int, string, bool, float, etc.
(primitive_type) @type.reference

; Union type parts: Foo|Bar
(union_type
  (named_type
    (name) @type.reference))
