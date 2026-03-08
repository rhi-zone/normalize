; MATLAB type references
; Captures type names used in class inheritance (superclass references).
;
; MATLAB class definitions use `<` to list superclasses:
;   classdef Foo < Bar & Baz
; The grammar represents each parent as a `superclass` node containing
; an `identifier`.

; Superclass reference in classdef: classdef Foo < Bar
(superclass
  (identifier) @type.reference)
