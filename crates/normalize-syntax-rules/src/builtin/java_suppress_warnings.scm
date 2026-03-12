# ---
# id = "java/suppress-warnings"
# severity = "info"
# tags = ["correctness", "cleanup"]
# message = "@SuppressWarnings found - fix the underlying warning instead of suppressing it"
# languages = ["java"]
# enabled = false
# ---
#
# `@SuppressWarnings` hides compiler warnings that often indicate real
# issues (unchecked casts, deprecation, resource leaks). Each suppressed
# warning is technical debt that may mask future bugs.
#
# ## How to fix
#
# Fix the underlying issue instead of suppressing it:
# ```java
# // Before
# @SuppressWarnings("unchecked")
# List<String> names = (List<String>) raw;
# // After
# List<String> names = new ArrayList<>();
# for (Object o : raw) { names.add((String) o); }
# ```
#
# ## When to disable
#
# Disabled by default (info severity). Some framework-generated code
# legitimately requires suppression annotations.

((annotation
  name: (identifier) @_name
  (#eq? @_name "SuppressWarnings")) @match)
