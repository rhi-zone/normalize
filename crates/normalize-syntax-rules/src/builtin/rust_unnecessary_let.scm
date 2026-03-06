# ---
# id = "rust/unnecessary-let"
# severity = "info"
# tags = ["style"]
# message = "Unnecessary let binding - consider using the value directly"
# languages = ["rust"]
# enabled = false
# ---
#
# `let x = y;` where both sides are simple identifiers (no destructuring,
# no transformation) creates an alias without adding meaning. The reader
# must now track two names that refer to the same value, which increases
# cognitive load without providing clarity.
#
# ## How to fix
#
# Use the original name directly. If the alias improves clarity (e.g.,
# `let config = self.config;` at the top of a method to avoid repeated
# field access), this is a judgment call — use the allow list.
#
# ## When to disable
#
# This rule is disabled by default (info severity). Intentional aliasing for
# readability is a legitimate use — disable per file or expression as needed.

; Detects: let x = y; where both are simple identifiers
; Excludes:
;   let mut (mutable)
;   underscore-prefixed names (unused markers)
;   None (Option variant)
;   uppercase RHS (unit structs / enum variants like `let x = MyStruct;`)
;   same-name rebindings (let x = x; — intentional move into closure)
;
; Known false positives (require dataflow to detect):
;   Index snapshot pattern: `let start = i; while i < ... { i += 1; }`
;   Here `start` captures a position before `i` changes — the alias is essential.
;   Cannot be excluded without dataflow analysis; accept or use the allow list.
(let_declaration
  (mutable_specifier)? @_mut
  pattern: (identifier) @_alias
  value: (identifier) @_value
  (#not-eq? @_mut "mut")
  (#not-match? @_alias "^_")
  (#not-eq? @_value "None")
  (#not-match? @_value "^[A-Z]")
  (#not-eq? @_alias @_value)) @match
