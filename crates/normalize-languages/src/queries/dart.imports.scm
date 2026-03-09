; Dart imports query
; @import       — the entire library import (for line number)
; @import.path  — the URI string

; import 'dart:collection';
(library_import
  (import_specification
    (uri
      (string_literal) @import.path))) @import

; import 'dart:collection' show Foo;
(library_import
  (import_specification
    (configurable_uri
      (uri
        (string_literal) @import.path)))) @import

; export 'uri';
(library_export
  (configurable_uri
    (uri
      (string_literal) @import.path))) @import
