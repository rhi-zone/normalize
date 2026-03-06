# normalize-derive/src

Single-file source for the `normalize-derive` proc-macro crate.

`lib.rs` implements `#[proc_macro_derive(Merge)]` using `syn` + `quote`. For named-field structs it emits `field: Merge::merge(self.field, other.field)` for each field; for tuple structs it indexes by position. Enum and union inputs produce a compile error. The generated impl references `::normalize_core::Merge` to avoid import requirements in the consumer.
