; D imports query
; @import       — the entire import declaration (for line number)
; @import.path  — the module being imported

; import math_utils;
(import_declaration
  (import_list
    (import
      (module_name) @import.path))) @import
