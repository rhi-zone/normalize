# ---
# id = "java/thread-sleep"
# severity = "warning"
# tags = ["bug-prone", "correctness"]
# message = "Thread.sleep() in production code is usually a sign of polling or timing issues"
# languages = ["java"]
# allow = ["**/test/**", "**/tests/**"]
# enabled = false
# ---
#
# `Thread.sleep()` blocks the current thread and is rarely the right
# solution in production code. It's usually a symptom of polling when
# an event-driven approach would be better, or a timing hack to work
# around race conditions.
#
# ## How to fix
#
# Use proper synchronization, scheduled executors, or reactive patterns:
# ```java
# // Before
# Thread.sleep(1000); // wait for resource
# // After
# CompletableFuture.supplyAsync(this::fetchResource)
#     .orTimeout(1, TimeUnit.SECONDS);
# ```
#
# ## When to disable
#
# Disabled by default. Test code is already excluded. Enable if you
# want to flag accidental sleeps in production code.

((method_invocation
  object: (identifier) @_obj
  name: (identifier) @_method
  (#eq? @_obj "Thread")
  (#eq? @_method "sleep")) @match)
