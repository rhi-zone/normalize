# ---
# id = "go/sync-mutex-copied"
# severity = "warning"
# tags = ["correctness", "concurrency"]
# message = "`sync.Mutex` (or `sync.RWMutex`) must not be copied after first use — pass a pointer instead"
# languages = ["go"]
# enabled = true
# recommended = true
# ---
#
# A `sync.Mutex` (and `sync.RWMutex`) must not be copied after first use.
# The mutex internal state (lock bits, waiter counts) is part of the struct
# value — copying it copies the current lock state, producing two independent
# mutexes that are each half-initialized, leading to deadlocks or races.
#
# `go vet` flags copies detected by data-flow analysis, but it only catches
# copies inside function bodies. This rule catches copies at the **signature
# level**: passing a mutex by value as a function parameter or returning one
# by value — both of which immediately produce a copy.
#
# ## How to fix
#
# Pass and return pointers:
#
# ```go
# // Bad — copies the mutex on every call:
# func unlock(mu sync.Mutex) {
#     mu.Unlock()
# }
#
# // Good — operates on the original:
# func unlock(mu *sync.Mutex) {
#     mu.Unlock()
# }
# ```
#
# If the mutex is embedded in a struct, pass a pointer to the struct
# (which is the standard Go pattern for any type containing a mutex).
#
# ## When to disable
#
# This rule is disabled by default (warning severity). The only legitimate
# reason to copy a sync type is in test code that deliberately exercises
# copy semantics. Use the allow list to exclude `*_test.go` files if needed.

; Mutex or RWMutex passed by value as a function parameter
(parameter_declaration
  type: (qualified_type
    package: (package_identifier) @_pkg
    name: (type_identifier) @_type)
  (#eq? @_pkg "sync")
  (#match? @_type "^(RW)?Mutex$")) @match

; Mutex or RWMutex returned by value from a function (single return type)
(function_declaration
  result: (qualified_type
    package: (package_identifier) @_pkg
    name: (type_identifier) @_type)
  (#eq? @_pkg "sync")
  (#match? @_type "^(RW)?Mutex$")) @match

(method_declaration
  result: (qualified_type
    package: (package_identifier) @_pkg
    name: (type_identifier) @_type)
  (#eq? @_pkg "sync")
  (#match? @_type "^(RW)?Mutex$")) @match
