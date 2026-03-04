; HCL tags query
; Covers: block definitions (resource, data, module, variable, output, locals, etc.)
; HCL blocks look like: resource "aws_instance" "example" { ... }
; The grammar models blocks as: (block (identifier) (string_lit)* (body))
; The first identifier is the block type; subsequent string_lit nodes are labels.

; Top-level blocks: capture the block type identifier as name
; e.g., resource "aws_s3_bucket" "example" { ... }
; e.g., variable "region" { ... }
; e.g., output "instance_ip" { ... }
(block
  (identifier) @name) @definition.var

; HCL attribute assignments at the top level
; e.g., locals { foo = "bar" } — the attribute name inside a block body
; Direct attribute bindings: name = value
(attribute
  (identifier) @name) @definition.var
