; CMake locals.scm
; function_def creates a scope. The first argument to function() is the name,
; remaining arguments are parameters. foreach() first argument is the loop var.
; set(VAR value) defines a variable. References use ${VAR} syntax.

; Scopes
; ------

(function_def) @local.scope

(foreach_loop) @local.scope

; Definitions
; -----------

; Function name and parameters: all arguments in function()
; (first = function name, rest = parameters; all captured as definitions)
(function_command
  (argument_list
    (argument
      (unquoted_argument) @local.definition)))

; foreach loop variable: first argument to foreach()
(foreach_command
  (argument_list .
    (argument
      (unquoted_argument) @local.definition)))

; set(VAR value): first argument is the variable name
((normal_command .
  (identifier) @_cmd
  (argument_list .
    (argument
      (unquoted_argument) @local.definition)))
 (#eq? @_cmd "set"))

; References
; ----------

; ${VAR} references
(variable_ref
  (normal_var
    (variable) @local.reference))
