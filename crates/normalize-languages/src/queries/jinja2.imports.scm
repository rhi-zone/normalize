; Jinja2 imports query
; @import       — the entire statement (for line number)
; @import.path  — the template path being referenced
;
; Captures: extends, import, from, include

; {% extends "base.html" %}
(statement
  (keyword) @_kw
  (string) @import.path
  (#eq? @_kw "extends")) @import

; {% import "macros.html" as m %}
(statement
  (keyword) @_kw
  (string) @import.path
  (#eq? @_kw "import")) @import

; {% from "helpers.html" import helper1 %}
(statement
  (keyword) @_kw
  (string) @import.path
  (#eq? @_kw "from")) @import

; {% include "header.html" %}
(statement
  (keyword) @_kw
  (string) @import.path
  (#eq? @_kw "include")) @import
