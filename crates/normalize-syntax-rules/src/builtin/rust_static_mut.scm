# ---
# id = "rust/static-mut"
# severity = "warning"
# tags = ["correctness", "concurrency"]
# message = "Mutable static variable - use a Mutex, RwLock, or OnceLock instead"
# languages = ["rust"]
# enabled = false
# ---
#
# `static mut` is `unsafe` to read or write because the compiler cannot
# guarantee exclusive access across threads. Any code that touches the variable
# must be wrapped in an `unsafe` block, bypassing Rust's safety guarantees at
# every call site.
#
# ## How to fix
#
# Replace with a thread-safe alternative:
# - `static FOO: OnceLock<T>` for write-once initialization
# - `static FOO: Mutex<T>` or `static FOO: RwLock<T>` for runtime mutation
# - `static FOO: AtomicUsize` (and friends) for primitive counters and flags
#
# ## When to disable
#
# This rule is disabled by default. FFI and low-level embedded code sometimes
# require `static mut` when interfacing with C APIs or hardware registers that
# have no safe wrapper. Add those sites to the allow list.

; Detects: static mut FOO: T = ...; declarations
(static_item
  (mutable_specifier)
  name: (identifier) @match)
