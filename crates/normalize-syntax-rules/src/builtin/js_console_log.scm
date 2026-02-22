# ---
# id = "js/console-log"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "console.log/debug found - remove before committing"
# languages = ["javascript", "typescript", "tsx", "jsx"]
# allow = ["**/tests/**", "**/*.test.*", "**/*.spec.*"]
# fix = ""
# enabled = false
# ---
#
# `console.log` and `console.debug` left in production JavaScript or
# TypeScript code pollute the browser console or server stdout, can leak
# sensitive data visible in DevTools, and make it impossible for callers
# to suppress the output.
#
# ## How to fix
#
# Remove the console call before committing. For structured production
# logging, use a library like `winston` or `pino` that supports log levels
# and output sinks. The auto-fix (`fix = ""`) deletes the entire statement.
#
# ## When to disable
#
# CLI tools and Node.js scripts that intentionally write to stdout are a
# legitimate exception. Disable per file or add to the allow list. This
# rule is disabled by default (info severity).

; Detects: console.log(), console.debug(), console.info() as full statement
((expression_statement
  (call_expression
    function: (member_expression
      object: (identifier) @_obj
      property: (property_identifier) @_prop)
    (#eq? @_obj "console")
    (#any-of? @_prop "log" "debug" "info"))) @match)
