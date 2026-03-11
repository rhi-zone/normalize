# ---
# id = "ruby/open-struct"
# severity = "warning"
# tags = ["performance", "architecture"]
# message = "`OpenStruct` uses `method_missing` and is slower than `Struct` or `Data` — prefer a named struct"
# languages = ["ruby"]
# enabled = false
# ---
#
# `OpenStruct` creates objects whose attributes are defined dynamically at
# runtime via `method_missing`. This means:
#
# - **Performance**: Every attribute access and assignment goes through
#   `method_missing`, which is significantly slower than a `Struct` or
#   plain `Data` class. Ruby core itself discourages `OpenStruct` in
#   performance-sensitive code.
# - **Discoverability**: The attributes of an `OpenStruct` instance are
#   invisible to static analysis, IDEs, and documentation tools. A `Struct`
#   or `Data` with named fields is self-documenting.
# - **Safety**: `OpenStruct` silently accepts any attribute name, making
#   typos invisible until runtime.
#
# ```ruby
# # Avoid:
# config = OpenStruct.new(debug: false, timeout: 30)
#
# # Prefer:
# Config = Struct.new(:debug, :timeout, keyword_init: true)
# config = Config.new(debug: false, timeout: 30)
#
# # Or (Ruby 3.2+):
# Config = Data.define(:debug, :timeout)
# config = Config.new(debug: false, timeout: 30)
# ```
#
# ## When to disable
#
# This rule is disabled by default (warning severity). `OpenStruct` is
# occasionally useful in tests or scripts where the flexibility outweighs
# the cost. Disable per file in those contexts.

; OpenStruct.new — dynamic struct via method_missing
(call
  receiver: (constant) @_class
  method: (identifier) @_method
  (#eq? @_class "OpenStruct")
  (#eq? @_method "new")) @match
