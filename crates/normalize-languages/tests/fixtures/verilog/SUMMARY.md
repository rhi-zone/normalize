# fixtures/verilog

Verilog fixture file for `.scm` query tests.

- `sample.v` — defines `alu` and `reg_file` modules with parameterized port declarations, `always` blocks with `case` statements, `assign` statements, and `localparam` constants; includes `(* synthesis, keep *)` attribute instance before `alu` to exercise `attribute_instance` decoration capture.
