; Scala imports query
; The Scala grammar splits dotted paths into separate identifier nodes in the
; path field. We capture just the import_declaration for line/presence detection,
; and rely on Language::extract_imports to parse the full path from text.
;
; This query exists so get_imports() returns Some, enabling the import path.
; The actual extraction is done by the trait method.

(import_declaration) @import
