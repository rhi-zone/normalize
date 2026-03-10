; HTML imports query
; @import       — the containing element (for line number)
; @import.path  — the URL/path being loaded

; <script src="app.js"></script>
(script_element
  (start_tag
    (attribute
      (attribute_name) @_attr
      (quoted_attribute_value) @import.path)
    (#eq? @_attr "src"))) @import

; <link href="styles.css"> (void element, implicit close)
(element
  (start_tag
    (tag_name) @_tag
    (attribute
      (attribute_name) @_attr
      (quoted_attribute_value) @import.path)
    (#eq? @_tag "link")
    (#eq? @_attr "href"))) @import

; <link href="styles.css" /> (self-closing syntax)
(element
  (self_closing_tag
    (tag_name) @_tag
    (attribute
      (attribute_name) @_attr
      (quoted_attribute_value) @import.path)
    (#eq? @_tag "link")
    (#eq? @_attr "href"))) @import
