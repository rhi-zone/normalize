; Verilog calls query
; @call — function/task call nodes
; @call.qualifier — not applicable
;
; Verilog/SystemVerilog has several call forms:
;   - tf_call: task/function call — contains simple_identifier or escaped_identifier
;   - system_tf_call: system task/function like $display, $finish
;   - method_call: method calls on objects (OOP SystemVerilog)

; Task/function call: some_func(args) or task_name;
(tf_call
  (simple_identifier) @call)

(tf_call
  (escaped_identifier) @call)

; System task/function call: $display(...), $finish
(system_tf_call
  (system_tf_identifier) @call)

; Method call: obj.method(args)
(method_call
  (method_call_body
    (method_identifier) @call))
