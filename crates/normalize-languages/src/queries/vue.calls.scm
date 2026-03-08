; Vue calls query
; @call — call expression nodes
; @call.qualifier — qualifier/receiver for method calls
;
; Note: The tree-sitter-vue grammar only models the Vue template structure
; (element, directive_attribute, interpolation, script_element, etc.). JavaScript
; content inside <script> blocks and template expressions is stored as raw_text
; and is NOT parsed into a JS AST by this grammar — call_expression nodes are
; not available here.
;
; Call extraction for Vue files depends on the injected JavaScript/TypeScript
; language (handled at a higher level via embedded_content). No query nodes are
; defined here.
