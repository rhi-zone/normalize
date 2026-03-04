; Type reference query for Ruby
; Ruby is dynamically typed; captures class names used in inheritance.

; Superclass in class definition: class Foo < Bar
(superclass
  (constant) @type.reference)

; Scope resolution: Foo::Bar — capture both parts
(scope_resolution
  (constant) @type.reference)
