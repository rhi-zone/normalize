# ---
# id = "no-todo-comment"
# severity = "info"
# message = "TODO comment found"
# enabled = false
# ---
#
# TODO comments are reminders that accumulate without accountability. Over
# time they form a graveyard of good intentions — things that were
# "temporary" years ago, references to people who left the team, and
# vague notes with no clear owner or timeline.
#
# ## How to fix
#
# Convert the TODO to a tracked issue in your issue tracker, then reference
# the issue number in a brief comment. This gives the work a real home with
# priority and assignee.
#
# ## When to disable
#
# This rule is disabled by default (info severity) because TODO comments are
# widely accepted in many teams and codebases. Enable it if you want to
# enforce a no-inline-todos policy.

; Matches line comments where TODO appears as a marker (followed by : or space),
; not as part of a filename like `TODO.md` or an identifier like `TODO_FILE`.
((line_comment) @match (#match? @match "TODO[: ]"))
