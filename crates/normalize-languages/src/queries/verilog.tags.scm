; Verilog tags query

; Module declarations — name is a simple_identifier child of module_header
(module_declaration
  (module_header
    (simple_identifier) @name)) @definition.module

; Function declarations — name is a function_identifier in function_body_declaration
(function_declaration
  (function_body_declaration
    (function_identifier
      (simple_identifier) @name))) @definition.function

; Task declarations — name is a task_identifier in task_body_declaration
(task_declaration
  (task_body_declaration
    (task_identifier
      (simple_identifier) @name))) @definition.function

; Class declarations — name is a class_identifier child
(class_declaration
  (class_identifier
    (simple_identifier) @name)) @definition.class
