; SCSS tags — mixins, functions, rule sets, at-rules, declarations

(mixin_statement
  name: (identifier) @name) @definition.function

(function_statement
  name: (identifier) @name) @definition.function

(rule_set
  (selectors) @name) @definition.class

(media_statement) @definition.module

(supports_statement) @definition.module

(keyframes_statement
  (keyframes_name) @name) @definition.module

(declaration
  (property_name) @name) @definition.variable
