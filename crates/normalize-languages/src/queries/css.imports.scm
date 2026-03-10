; CSS imports query
; @import       — the entire @import statement (for line number)
; @import.path  — the stylesheet path being imported

; @import "file.css";
(import_statement
  (string_value) @import.path) @import

; @import url("file.css");
(import_statement
  (call_expression
    (arguments
      (string_value) @import.path))) @import

; @import url(file.css);  (bare URL without quotes)
(import_statement
  (call_expression
    (arguments
      (plain_value) @import.path))) @import
