; jq CFG query
; Captures control flow nodes for CFG construction.
; See normalize-cfg for the full capture vocabulary.
; Verified against arborium jq grammar node types.
;
; In jq, if/then/else/elif/end are tokens, not named statement nodes.
; The grammar uses unnamed nodes for if/else branches.
; try/catch are available as expression forms.

; ---------------------------------------------------------------------------
; elif (branch continuation — the only named branch node)
; ---------------------------------------------------------------------------

(elif) @cfg.branch

; ---------------------------------------------------------------------------
; try / catch
; ---------------------------------------------------------------------------

(catch) @cfg.try.catch
