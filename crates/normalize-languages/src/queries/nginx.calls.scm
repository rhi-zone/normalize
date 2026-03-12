; Nginx calls query
; @call — directive name (nginx directives are effectively function calls)
; @call.qualifier — not applicable (nginx has no method receiver concept)
;
; In nginx configs, directives like `proxy_pass`, `listen`, `server_name` etc.
; are effectively calls to built-in functions. Both simple directives
; (e.g. `proxy_pass http://backend;`) and block directives (e.g. `server { ... }`)
; have a directive name that acts as the "function" being called.

; Simple directive: proxy_pass http://backend;
(simple_directive
  name: (directive) @call)

; Block directive: server { ... }, location /api { ... }
(block_directive
  name: (directive) @call)
