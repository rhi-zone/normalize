; Clojure is homoiconic: all special forms (defn, fn, let, etc.) are list_lit
; nodes distinguished only by their leading sym_lit. Predicates (#any-of?) on
; named sym_lit captures are evaluated automatically by QueryCursor::matches.

; Scopes
; ------

; def forms create lexical scopes
((list_lit . (sym_lit) @_kw) @local.scope
 (#any-of? @_kw
   "defn" "defn-" "defmacro"
   "fn"
   "let" "loop"
   "when-let" "if-let" "when-some" "if-some"))

; Anonymous fn shorthand #(...) creates a scope
(anon_fn_lit) @local.scope

; Definitions
; -----------

; Named function: (defn foo [...] ...)
; Captures the sym_lit immediately following defn/defmacro.
((list_lit . (sym_lit) @_kw . (sym_lit) @local.definition)
 (#any-of? @_kw "defn" "defn-" "defmacro"))

; Parameters (fn, defn) and bindings (let, loop, when-let, etc.):
; all sym_lits directly inside the binding/param vector.
; Note: in let, value-position sym_lits are also captured — unavoidable
; without index-based predicates.
((list_lit . (sym_lit) @_kw
            (vec_lit (sym_lit) @local.definition))
 (#any-of? @_kw
   "defn" "defn-" "fn" "loop"
   "let" "when-let" "if-let" "when-some" "if-some"))

; References
; ----------

(sym_lit) @local.reference
