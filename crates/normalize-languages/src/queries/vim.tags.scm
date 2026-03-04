; Vim tags query
; Covers: function definitions and augroup definitions

; Function definitions: function! FunctionName(args) ... endfunction
; The function_definition node contains a function_declaration with a name field.
(function_definition
  (function_declaration
    name: (identifier) @name)) @definition.function

; Scoped function: function! foo#bar#Baz() — scoped_identifier
(function_definition
  (function_declaration
    name: (scoped_identifier) @name)) @definition.function

; Augroup definitions: augroup MyGroup ... augroup END
; The augroup_statement node contains an augroup_name child.
(augroup_statement
  (augroup_name) @name) @definition.module
