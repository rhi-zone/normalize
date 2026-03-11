; CSS symbols: selectors as classes, at-rules as modules, declarations as variables.

(rule_set
  (selectors) @name) @definition.class

(media_statement) @definition.module

(supports_statement) @definition.module

(keyframes_statement
  (keyframes_name) @name) @definition.function

(declaration
  (property_name) @name) @definition.var
