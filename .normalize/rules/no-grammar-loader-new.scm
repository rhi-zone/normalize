# ---
# id = "no-grammar-loader-new"
# severity = "error"
# message = "Use grammar_loader() singleton instead of GrammarLoader::new"
# allow = ["**/parsers.rs", "**/registry.rs", "**/grammar_loader.rs", "**/python.rs"]
# ---

(call_expression
  function: (scoped_identifier
    path: (identifier) @_type
    name: (identifier) @_method)
  (#eq? @_type "GrammarLoader")
  (#eq? @_method "new")) @match
