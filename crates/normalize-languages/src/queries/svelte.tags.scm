; Svelte tags — top-level script and style blocks
; Svelte SFCs embed JS in <script> and CSS in <style>.
; The blocks themselves are captured as containers; JS symbol extraction
; happens via language injection in the embedded content.

(script_element
  (start_tag
    (tag_name) @name)) @definition.module

(style_element
  (start_tag
    (tag_name) @name)) @definition.module
