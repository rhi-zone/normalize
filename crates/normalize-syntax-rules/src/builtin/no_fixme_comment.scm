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

; Matches line comments containing FIXME
; Works across languages with line_comment node type
((line_comment) @match (#match? @match "FIXME"))
