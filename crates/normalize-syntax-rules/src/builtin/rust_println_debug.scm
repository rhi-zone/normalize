# ---
# id = "rust/println-debug"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "println!/print! found - consider using tracing or log crate"
# languages = ["rust"]
# allow = ["**/tests/**", "**/examples/**", "**/bin/**", "**/main.rs"]
# enabled = false
# ---
#
# `println!` and `print!` write directly to stdout, which callers of a
# library cannot suppress or redirect. They also produce unstructured output
# with no severity level, timestamps, or context — making them useless for
# production observability.
#
# ## How to fix
#
# Use the `log` or `tracing` crate instead. Callers can then configure log
# levels and output sinks without modifying library code.
#
# ## When to disable
#
# Binary entry points (main.rs, bin/**) that intentionally write to stdout
# are already excluded in the default allow list. This rule is disabled by
# default (info severity).

((macro_invocation
  macro: (identifier) @_name
  (#any-of? @_name "println" "print" "eprintln" "eprint")) @match)
