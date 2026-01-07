# ---
# id = "rust/unnecessary-let"
# severity = "info"
# message = "Unnecessary let binding - consider using the value directly"
# languages = ["rust"]
# ---

; Detects: let x = y; where both are simple identifiers
; Excludes: let mut (mutable), underscore-prefixed names, None (Option variant)
; This may be intentional for clarity, so severity is info
(let_declaration
  (mutable_specifier)? @_mut
  pattern: (identifier) @_alias
  value: (identifier) @_value
  (#not-eq? @_mut "mut")
  (#not-match? @_alias "^_")
  (#not-eq? @_value "None")) @match
