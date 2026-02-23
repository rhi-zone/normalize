# ---
# id = "ruby/global-var"
# severity = "warning"
# tags = ["architecture", "mutable-state", "global-state", "mutable-global-state"]
# message = "Global variable ($var) - use a class, module, or explicit parameter instead"
# languages = ["ruby"]
# enabled = false
# ---
#
# Ruby global variables (`$name`) are mutable state shared across the entire
# process. Any code anywhere can read or overwrite them, making behaviour
# dependent on call order and making isolation in tests impossible without
# explicit teardown. They also conflict with Ruby's built-in globals
# (`$stdout`, `$LOAD_PATH`, etc.), creating surprising name-collision risks.
#
# ## How to fix
#
# - Encapsulate the state in a module or class and access it through an
#   explicit interface (`Config.value` rather than `$config`).
# - Pass the value as a parameter to the code that needs it.
# - Use `Thread.current[:key]` for thread-local state if isolation is the goal.
#
# ## When to disable
#
# This rule is disabled by default. Gems that intentionally extend Ruby's
# built-in globals (e.g., `$stdout` redirection for testing) are a legitimate
# use case. Add those sites to the allow list.

; Detects assignments to Ruby global variables ($name)
(assignment
  left: (global_variable) @match)
