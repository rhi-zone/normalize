# normalize-openapi/src

Source for the normalize-openapi crate.

`lib.rs` contains the entire crate: the `OpenApiClientGenerator` trait, the `OnceLock`/`RwLock`-backed global registry, the three built-in generator structs (`TypeScriptFetch`, `PythonUrllib`, `RustUreq`), and shared JSON Schema type-mapping helpers (`json_schema_to_ts`, `json_schema_to_py`, `json_schema_to_rust`).
