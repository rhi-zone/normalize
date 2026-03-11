; Dockerfile tags query
; @name            — the symbol name
; @definition.*    — the definition node

; Build stage names (FROM ... AS name)
(from_instruction
  (image_alias) @name) @definition.module

; ARG declarations
(arg_instruction
  (unquoted_string) @name) @definition.constant

; ENV declarations
(env_instruction
  (env_pair
    (unquoted_string) @name)) @definition.constant
