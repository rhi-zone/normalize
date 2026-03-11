# ---
# id = "rust/commented-out-code"
# severity = "info"
# tags = ["cleanup"]
# message = "Comment looks like disabled code - remove or restore it"
# languages = ["rust"]
# enabled = false
# ---
#
# Commented-out code accumulates silently and rots. It confuses readers who
# wonder whether it was disabled temporarily, is a TODO, or was simply
# forgotten. Version control preserves history — if code needs to come back,
# `git log` will have it.
#
# ## How to fix
#
# Delete the commented-out code. If it's needed later, find it in version
# control history. If it documents intent, rewrite it as a prose comment
# explaining *why*, not *what*.
#
# ## When to disable
#
# This rule is disabled by default (info severity) because some teams
# intentionally keep commented-out alternatives as documentation. Enable it
# if you want to enforce a clean-comments policy.

; Matches line comments whose content looks like a Rust code statement:
; - return/if/while followed by syntax chars
; - use/let/fn/pub/impl/mod/struct/enum declarations
; - function calls ending with semicolons
; - assignments (x = value, not x == value)
((line_comment) @match
 (#match? @match "^//\\s*(return\\s+\\S|if\\s*[\\({]|while\\s|use\\s+\\w|let\\s+\\w|fn\\s+\\w|pub\\s|impl\\s|mod\\s+\\w|struct\\s+\\w|enum\\s+\\w|[a-zA-Z_][\\w:.]*\\s*\\(.*\\)\\s*;|[a-zA-Z_]\\w*\\s*=[^=])"))
