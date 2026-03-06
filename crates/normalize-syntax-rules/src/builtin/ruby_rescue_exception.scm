# ---
# id = "ruby/rescue-exception"
# severity = "warning"
# tags = ["error-handling"]
# message = "`rescue Exception` catches too broadly - rescue `StandardError` instead"
# languages = ["ruby"]
# enabled = false
# ---
#
# In Ruby, `rescue Exception` catches every exception in the hierarchy,
# including `SignalException` (which handles Unix signals), `Interrupt`
# (Ctrl+C), `NoMemoryError`, `SystemExit`, and `ScriptError`. These are
# not recoverable errors — they indicate that the runtime itself is
# shutting down or responding to an OS-level event.
#
# Catching `Exception` prevents the program from being interrupted with
# Ctrl+C, interferes with `exit` and `abort`, and can mask out-of-memory
# conditions that should terminate the process.
#
# ## How to fix
#
# Replace `rescue Exception` with `rescue StandardError` (or a more specific
# subclass). `StandardError` is the base class for all application-level
# errors and is the implicit default when no exception class is listed:
#
# ```ruby
# begin
#   do_something
# rescue StandardError => e   # or just: rescue => e
#   handle(e)
# end
# ```
#
# ## When to disable
#
# This rule is disabled by default (warning severity). Top-level exception
# reporters and daemon harnesses that genuinely must survive every condition
# may need `rescue Exception`. Disable per site with an allow comment.

; Detects rescue Exception — catches non-recoverable exceptions
(rescue
  (exceptions
    (constant) @_exc
    (#eq? @_exc "Exception"))) @match
