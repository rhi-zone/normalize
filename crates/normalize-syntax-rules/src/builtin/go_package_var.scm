# ---
# id = "go/package-var"
# severity = "info"
# tags = ["architecture", "concurrency", "mutable-global-state"]
# message = "Package-level var - mutable global state is a concurrency and testability hazard"
# languages = ["go"]
# enabled = false
# ---
#
# Package-level `var` declarations are mutable global state shared across all
# goroutines. Concurrent reads and writes require explicit synchronization;
# without it the program has a data race. They also make packages harder to
# test because state persists between test cases unless explicitly reset.
#
# Package-level `const` and `var` holding interface values used for dependency
# injection are a common Go pattern, but mutable vars in that role are still
# a smell — they imply side-channel wiring rather than explicit dependencies.
#
# ## How to fix
#
# - Pass dependencies as parameters or struct fields rather than relying on
#   package-level state.
# - If shared state is unavoidable, protect it with `sync.Mutex` or
#   `sync/atomic` and document the invariants.
# - For package-level constants (e.g., default values), use `const` or a
#   function returning a new value each time.
#
# ## When to disable
#
# This rule is disabled by default (info severity). `var` is idiomatic for
# package-level error sentinels (`var ErrNotFound = errors.New(...)`) and
# `sync.Once` / `sync.Mutex` guards. Add those patterns to the allow list.

; Detects: var declarations at package (source file) level
(source_file
  (var_declaration
    (var_spec
      name: (identifier) @match)))
