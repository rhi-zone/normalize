; DeviceTree imports query
; @import       — the entire #include directive (for line number)
; @import.path  — the header/overlay file path

; #include "board.dtsi"
(preproc_include
  path: (string_literal) @import.path) @import

; #include <dt-bindings/gpio/gpio.h>
(preproc_include
  path: (system_lib_string) @import.path) @import
