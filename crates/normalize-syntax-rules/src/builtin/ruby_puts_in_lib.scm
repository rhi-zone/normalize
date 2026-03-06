# ---
# id = "ruby/puts-in-lib"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "`puts` in library code writes to stdout - use a logger or raise instead"
# languages = ["ruby"]
# allow = ["**/bin/**", "**/exe/**", "**/*_spec.rb", "**/spec/**", "**/test/**", "Rakefile", "**/Rakefile"]
# enabled = false
# ---
#
# `puts` writes directly to `$stdout` with no way for callers to suppress,
# redirect, or filter the output. In library code this violates the principle
# of least surprise: callers cannot silence the output even if they configure
# a logger or redirect `$stdout` before calling your code, because `puts`
# bypasses custom logging entirely.
#
# `puts` is also invisible in test output — it does not appear in the test
# reporter and cannot be asserted against, making debugging harder.
#
# ## How to fix
#
# For diagnostic output: use a logger passed in via dependency injection, or
# the `Logger` stdlib:
#
# ```ruby
# logger.info("message")
# ```
#
# For error conditions: raise an exception instead of printing and continuing.
#
# For intentional user-facing output in scripts: move the `puts` call to the
# CLI entry point (`bin/` or `exe/`) rather than the library layer.
#
# ## When to disable
#
# CLI entry points (`bin/`, `exe/`), test files, and Rakefiles are already
# excluded in the default allow list. This rule is disabled by default (info
# severity) because scripts and one-off utilities legitimately use `puts`.

; Detects: puts "..." — bare puts call (not a method call on a receiver)
(call
  method: (identifier) @_method
  (#eq? @_method "puts")) @match
