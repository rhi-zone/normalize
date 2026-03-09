; Nginx imports query
; @import       — the entire include directive (for line number)
; @import.path  — the path pattern being included

; include /etc/nginx/conf.d/*.conf;
(simple_directive
  name: (directive) @_name
  (param) @import.path
  (#eq? @_name "include")) @import
