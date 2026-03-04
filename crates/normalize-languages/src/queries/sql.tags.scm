; SQL tags — definitions for functions, tables, views, schemas, types

(create_function
  (object_reference) @name) @definition.function

(create_table
  (object_reference) @name) @definition.class

(create_view
  (object_reference) @name) @definition.class

(create_schema
  (identifier) @name) @definition.module

(create_type
  (object_reference) @name) @definition.type
