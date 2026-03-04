; Nginx imports query
; @import       — the entire include directive (for line number)
; @import.path  — the path pattern being included

; include /etc/nginx/conf.d/*.conf;
(directive
  (identifier) @_name
  (#eq? @_name "include")
  (string) @import.path) @import
