# ---
# id = "csharp/console-write"
# severity = "info"
# tags = ["debug-print", "cleanup"]
# message = "Console.Write/WriteLine found - consider using a logging framework"
# languages = ["c-sharp"]
# allow = ["**/test/**", "**/tests/**", "**/examples/**"]
# enabled = false
# ---
#
# `Console.WriteLine` and related methods write directly to stdout/stderr.
# In production code, use a structured logging framework (Serilog, NLog,
# Microsoft.Extensions.Logging) for configurability and log levels.
#
# ## How to fix
#
# ```csharp
# // Before
# Console.WriteLine($"User {id} logged in");
# // After
# _logger.LogInformation("User {UserId} logged in", id);
# ```
#
# ## When to disable
#
# Disabled by default. Enable if you want to flag console output in
# production code. Test and example directories are already excluded.

((invocation_expression
  function: (member_access_expression
    expression: (identifier) @_obj
    name: (identifier) @_method)
  (#eq? @_obj "Console")
  (#match? @_method "^(WriteLine|Write|ReadLine|ReadKey)$")) @match)
