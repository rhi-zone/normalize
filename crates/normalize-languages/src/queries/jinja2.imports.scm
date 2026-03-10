; Jinja2 imports query for tree-sitter-jinja2 (normalize grammar)
; @import       — the entire statement (for line number)
; @import.path  — the template path being referenced
;
; Captures: extends, import, from, include

; {% extends "base.html" %}
(extends_statement
  path: (string) @import.path) @import

; {% import "macros.html" as m %}
(import_statement
  path: (string) @import.path) @import

; {% from "helpers.html" import helper1 %}
(from_statement
  path: (string) @import.path) @import

; {% include "header.html" %}
; {% include "optional.html" ignore missing %}
(include_statement
  path: (string) @import.path) @import
