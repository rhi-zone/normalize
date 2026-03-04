; Agda tags query

; Function definitions (lhs contains a function_name child which contains id)
(function
  (lhs
    (function_name) @name)) @definition.function

; Type signatures (function type declarations without body)
(signature
  (signature
    (function_name) @name)) @definition.function

; Data type declarations
(data
  (data_name) @name) @definition.type

; Record type declarations
(record
  (record_name) @name) @definition.class

; Module declarations
(module
  (module_name) @name) @definition.module
