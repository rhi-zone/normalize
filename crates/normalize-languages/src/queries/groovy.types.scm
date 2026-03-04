; Type reference query for Groovy
; Captures type identifiers used in declarations and parameters.

; Qualified type names: Foo, groovy.lang.Closure
(qualified_name) @type.reference

; Generic types: List<String>, Map<String, Object>
(type_with_generics
  (qualified_name) @type.reference)
