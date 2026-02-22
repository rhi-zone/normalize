# ---
# id = "python/breakpoint"
# severity = "warning"
# tags = ["debug-print", "cleanup"]
# message = "breakpoint() found - remove before committing"
# languages = ["python"]
# allow = ["**/tests/**"]
# fix = ""
# ---
#
# `breakpoint()` drops into the Python debugger (pdb or a configured
# alternative) at runtime, halting program execution and waiting for
# interactive input. If committed, it will block any environment that runs
# the code — including CI, production servers, and automated tests.
#
# ## How to fix
#
# Remove the `breakpoint()` call. The auto-fix (`fix = ""`) deletes the
# entire statement. If you need post-mortem debugging, use
# `sys.excepthook` or configure your runner to break on exceptions.
#
# ## When to disable
#
# Never — a committed `breakpoint()` will block execution in any non-TTY
# environment.

((call
  function: (identifier) @_name
  (#eq? @_name "breakpoint")) @match)
