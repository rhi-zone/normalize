# ---
# id = "ruby/method-missing"
# severity = "warning"
# tags = ["correctness"]
# message = "method_missing defined without respond_to_missing? — objects won't respond correctly to respond_to?"
# languages = ["ruby"]
# enabled = true
# recommended = true
# ---
#
# When you override `method_missing` in Ruby, you should also override
# `respond_to_missing?`. Without it, `respond_to?` returns `false` for
# methods that `method_missing` handles, breaking duck-typing contracts
# and introspection tools.
#
# ```ruby
# # Bad — respond_to?(:foo) returns false even though foo works:
# class Proxy
#   def method_missing(name, *args)
#     @target.send(name, *args)
#   end
# end
#
# # Good — respond_to? is consistent with method_missing:
# class Proxy
#   def method_missing(name, *args)
#     @target.send(name, *args)
#   end
#
#   def respond_to_missing?(name, include_private = false)
#     @target.respond_to?(name, include_private) || super
#   end
# end
# ```
#
# ## How to fix
#
# Define `respond_to_missing?` alongside `method_missing`. It should
# return `true` for the same methods that `method_missing` handles,
# and delegate to `super` otherwise.
#
# ## When to disable
#
# This rule is disabled by default. Tree-sitter queries cannot check
# for sibling method definitions, so this fires on every `method_missing`
# definition. Disable for files where `respond_to_missing?` is known to
# be defined (e.g., in a sibling module or via metaprogramming).

(method
  name: (identifier) @_name
  (#eq? @_name "method_missing")) @match
