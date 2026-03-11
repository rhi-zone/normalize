# ---
# id = "commented-out-code"
# severity = "info"
# tags = ["cleanup"]
# message = "Comment looks like disabled code - remove or restore it"
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
#
# Works with languages that use `comment` node type (Python, JavaScript,
# TypeScript, Go, Ruby, and most others). Rust uses `line_comment` and
# requires a separate rule.

; Matches comments whose content looks like a code statement:
; - return/if/while followed by syntax chars
; - import/let/var/const/def/fn/func declarations
; - function calls ending with semicolons
; - assignments (x = value, not x == value)
((comment) @match
 (#match? @match "^(//|#)\\s*(return\\s+\\S|if\\s*\\(|while\\s*\\(|import\\s+\\w|let\\s+\\w+\\s*=|var\\s+\\w+\\s*=|const\\s+\\w+\\s*=|def\\s+\\w+\\s*\\(|fn\\s+\\w+\\s*\\(|func\\s+\\w+\\s*\\(|[a-zA-Z_][\\w.]*\\s*\\(.*\\)\\s*;)"))
