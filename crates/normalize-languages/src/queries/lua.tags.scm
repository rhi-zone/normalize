; Lua tags query
; Covers: function declarations, local function declarations, method assignments

; Global function declarations: function foo(...) ... end
(function_declaration
  name: (identifier) @name) @definition.function

; Local function declarations: local function foo(...) ... end
(local_function
  name: (identifier) @name) @definition.function

; Method definitions via assignment: function Table:method(...) ... end
; or: Namespace.method = function(...) ... end
(assignment_statement
  (variable_list
    (dot_index_expression
      field: (identifier) @name))
  (expression_list
    (function_definition))) @definition.method

; Module-style method: Foo:bar = function(...)
(assignment_statement
  (variable_list
    (method_index_expression
      method: (identifier) @name))
  (expression_list
    (function_definition))) @definition.method
