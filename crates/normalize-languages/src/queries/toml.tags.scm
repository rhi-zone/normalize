; TOML structure: tables/array tables as containers, pairs as variables.
; Inline table inner pairs are filtered out via node_name() in Rust.

(table
  (bare_key) @name) @definition.class

(table_array_element
  (bare_key) @name) @definition.class

(pair
  (bare_key) @name) @definition.var
