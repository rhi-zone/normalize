;; In tree-sitter-idris, ||| doc comments are parsed as (comment) by the external scanner — there is no separate doc_comment node.
;; Pragmas (%hint, %inline, etc.) are declaration-level nodes (pragma_hint, pragma_inline, etc.), not a generic (pragma) wrapper.
;; Both are therefore captured via (comment).
(comment) @decoration
