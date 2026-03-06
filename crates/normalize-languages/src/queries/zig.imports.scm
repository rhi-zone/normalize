; Zig imports query
; @import       — the entire @import call (for line number)
; @import.path  — the module path string (quotes stripped by Rust)
;
; In Zig, @import calls look like:
;   const std = @import("std");
;   const math_utils = @import("./math_utils.zig");
;
; The AST structure is:
;   VarDecl -> ErrorUnionExpr -> SuffixExpr -> BUILTINIDENTIFIER + FnCallArguments -> STRINGLITERALSINGLE

; @import("std") or @import("./file.zig")
; The SuffixExpr node contains BUILTINIDENTIFIER as an unnamed child and FnCallArguments
(SuffixExpr
  (BUILTINIDENTIFIER) @_f (#eq? @_f "@import")
  (FnCallArguments
    (ErrorUnionExpr
      (SuffixExpr
        (STRINGLITERALSINGLE) @import.path)))) @import
