# ---
# id = "ruby/binding-pry"
# severity = "warning"
# tags = ["debug-print", "cleanup"]
# message = "binding.pry found - remove debug statement before committing"
# languages = ["ruby"]
# allow = ["**/tests/**", "**/test/**", "**/spec/**"]
# fix = ""
# ---
#
# `binding.pry` and `binding.irb` drop into an interactive Ruby debugger
# at runtime, halting program execution. If committed and hit in a
# non-interactive environment — CI, a server, a background job — the
# process hangs indefinitely waiting for input that will never come.
#
# ## How to fix
#
# Remove the `binding.pry` or `binding.irb` call before committing. The
# auto-fix (`fix = ""`) deletes the entire statement. Use proper logging or
# exception handling for production debugging.
#
# ## When to disable
#
# Never — a committed `binding.pry` will hang any non-TTY process that
# reaches that line.

((call
  receiver: (identifier) @_receiver
  method: (identifier) @_method
  (#eq? @_receiver "binding")
  (#any-of? @_method "pry" "irb")) @match)
