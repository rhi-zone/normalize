; CMake tags query
;
; CMake functions and macros use: function(name args) ... endfunction()
; The name is the first argument after the `function` or `macro` keyword.

(function_def
  (function_command
    (argument_list
      .
      (argument) @name))) @definition.function

(macro_def
  (macro_command
    (argument_list
      .
      (argument) @name))) @definition.function
