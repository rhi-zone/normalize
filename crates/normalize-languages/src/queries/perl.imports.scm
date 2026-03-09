; Perl imports query
; @import       — the entire import statement (for line number)
; @import.path  — the module path being imported

; use Module::Name;
(use_statement
  module: (package) @import.path) @import

; require Module::Name; or require 'file.pl';
(require_expression) @import
