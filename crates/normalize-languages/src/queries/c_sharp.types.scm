; C# type references
; Captures identifiers used in type positions.

; Simple type names: Foo, List, string, int
(identifier) @type.reference

; Qualified names: System.Collections.Generic.List
(qualified_name) @type.reference

; Generic names: List<T>, Dictionary<K,V>
(generic_name
  (identifier) @type.reference)
