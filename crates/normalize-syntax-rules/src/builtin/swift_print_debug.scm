# ---
# id = "swift/print-debug"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "print/debugPrint/NSLog found - consider using os_log or a logging framework"
# languages = ["swift"]
# allow = ["**/test/**", "**/tests/**", "**/examples/**", "**/*Playground*/**"]
# enabled = false
# ---
#
# `print`, `debugPrint`, and `NSLog` are fine for debugging but should
# not ship in production. Use `os_log` or swift-log for structured,
# level-aware logging.
#
# ## How to fix
#
# ```swift
# // Before
# print("User \(id) logged in")
# // After
# logger.info("User \(id, privacy: .public) logged in")
# ```
#
# ## When to disable
#
# Disabled by default. Enable if you want to flag console output in
# production code.

((call_expression
  (simple_identifier) @_fn
  (#match? @_fn "^(print|debugPrint|NSLog|dump)$")) @match)
