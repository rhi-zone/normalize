; Svelte calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; Note: The tree-sitter-svelte grammar only models the Svelte template structure
; (if_statement, each_statement, expression_tag, script_element, etc.). JavaScript
; content inside <script> blocks and event handlers is stored as raw_text and is
; NOT parsed into a JS AST by this grammar — call_expression nodes are not
; available here.
;
; Call extraction for Svelte files depends on the injected JavaScript/TypeScript
; language (handled at a higher level via embedded_content). No query nodes are
; defined here.
