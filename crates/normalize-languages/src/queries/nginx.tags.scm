; Nginx tags query
; Covers: block directives (server, location, upstream, http, events, etc.)
; Nginx config is structured as block_directive nodes with a name: (directive) field.
; e.g., server { ... }, location /api { ... }, upstream backend { ... }

; Block directives: named blocks that form the top-level structure
(block_directive
  name: (directive) @name) @definition.module
