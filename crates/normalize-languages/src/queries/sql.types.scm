; SQL types query
; @type — type references in column definitions and CREATE TYPE statements
;
; SQL has static type information in DDL statements. Column definitions
; carry an explicit data type (INTEGER, VARCHAR, TEXT, etc.), and CREATE TYPE
; defines named composite/enum/domain types.

; Column type in CREATE TABLE: col_name INTEGER
(column_definition
  type: (_) @type)

; Column type via custom_type (user-defined type reference): col_name my_type
(column_definition
  custom_type: (object_reference) @type)
