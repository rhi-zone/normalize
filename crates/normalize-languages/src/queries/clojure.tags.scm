; Clojure tags query
;
; Clojure is a Lisp — all forms are list_lit nodes.
; Definitions use leading sym_lit: defn, defmacro, ns, defrecord, defprotocol, def.

; (defn name [...] ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defn")
  .
  (sym_lit) @name) @definition.function

; (defn- name [...] ...) — private function
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defn-")
  .
  (sym_lit) @name) @definition.function

; (defmacro name [...] ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defmacro")
  .
  (sym_lit) @name) @definition.macro

; (defmethod name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defmethod")
  .
  (sym_lit) @name) @definition.method

; (ns name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "ns")
  .
  (sym_lit) @name) @definition.module

; (defrecord Name [...] ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defrecord")
  .
  (sym_lit) @name) @definition.class

; (deftype Name [...] ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "deftype")
  .
  (sym_lit) @name) @definition.class

; (defprotocol Name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "defprotocol")
  .
  (sym_lit) @name) @definition.interface

; (def name ...)
(list_lit
  (sym_lit) @_kw (#eq? @_kw "def")
  .
  (sym_lit) @name) @definition.constant
