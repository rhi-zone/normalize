; TLA+ calls query
; @call — operator/procedure application nodes
; @call.qualifier — not applicable
;
; TLA+ (and PlusCal) has two call-like constructs:
;   - pcal_macro_call: PlusCal macro invocation
;   - pcal_proc_call: PlusCal procedure call
; Both have a `name` field with an `identifier`.

; PlusCal macro call: call MacroName(args)
(pcal_macro_call
  name: (identifier) @call)

; PlusCal procedure call: call ProcName(args)
(pcal_proc_call
  name: (identifier) @call)
