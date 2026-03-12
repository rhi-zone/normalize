; Caddy tags query
; Covers: site blocks (virtual hosts), snippets, named matchers, handle/route directives
; The tree-sitter-caddy grammar models Caddyfiles as:
;   document -> site_block (site_address, directive_block, ...)
;            -> snippet (snippet_name, directive_block, ...)

; Site blocks: the primary organizational unit — one per virtual host
; e.g., example.com { ... }, :8443 { ... }
(site_block
  (site_address) @name) @definition.module

; Snippets: reusable named configuration blocks
; e.g., (common-headers) { ... }
(snippet
  (snippet_name) @name) @definition.module

; Named matchers: define request matching criteria referenced by directives
; e.g., @api { path /api/* }
(matcher_definition
  (matcher_name) @name) @definition.var

; Handle directives with a named matcher: route-like blocks that group directives
; e.g., handle @api { reverse_proxy ... }
; "handle" and "route" are anonymous keyword nodes, so we capture matcher_token as name
(directive_handle
  (matcher_token) @name) @definition.function

; Route directives with a matcher: explicit ordering blocks
; e.g., route /docs/* { file_server }
(directive_route
  (matcher_token) @name) @definition.function
