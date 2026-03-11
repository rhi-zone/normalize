# ---
# id = "no-fixme-comment"
# severity = "warning"
# tags = ["cleanup"]
# message = "FIXME comment found - fix before merging"
# ---
#
# FIXME comments mark code that the author knew was broken or incorrect at
# the time of writing. Merging a FIXME is an explicit acknowledgment that
# broken code is being shipped. Unlike TODO, FIXME implies an active defect,
# not just future work.
#
# ## How to fix
#
# Fix the underlying issue before merging. If fixing it now is not feasible,
# convert the FIXME to a tracked issue in your issue tracker and replace the
# comment with a reference to the issue number.
#
# ## When to disable
#
# If your team uses FIXME as a pre-release review marker and has a defined
# process for clearing them before shipping, you can disable this rule.

; Matches comments containing FIXME
; Three patterns to cover all tree-sitter comment node types:
; `comment` (Python, JS, Go, Ruby, Java), `line_comment` (Rust, C, C++),
; `block_comment` (Rust `/* ... */`, C/C++ block comments).
((comment) @match (#match? @match "FIXME"))
((line_comment) @match (#match? @match "FIXME"))
((block_comment) @match (#match? @match "FIXME"))
