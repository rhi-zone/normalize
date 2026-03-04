; Type reference query for F#
; Captures type identifiers used in type annotations and definitions.

; Simple types: int, string, MyType
(simple_type
  (long_identifier) @type.reference)

; Generic types: List<int>, Option<string>
(generic_type
  (long_identifier) @type.reference)
