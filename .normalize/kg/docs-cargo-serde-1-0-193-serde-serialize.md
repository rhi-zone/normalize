---
fetched_at: 2026-05-27T08:14:11.095395728+00:00
item_kind: trait
kind: docs
language: rust
package: serde
source_url: https://docs.rs/serde/1.0.193/serde/trait.Serialize.html
symbol_path: serde::Serialize
version: 1.0.193
links:
- kind: source
  to: https://docs.rs/serde/1.0.193/serde/trait.Serialize.html
---
# serde::Serialize (rust, serde 1.0.193)

trait

```rust
pub trait Serialize {
    // Required method
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
       where S: Serializer;
}
```

A **data structure** that can be serialized into any data format supported
by Serde.


Serde provides `Serialize` implementations for many Rust primitive and
standard library types. The complete list is here. All of
these can be serialized using Serde out of the box.


Additionally, Serde provides a procedural macro called `serde_derive` to
automatically generate `Serialize` implementations for structs and enums in
your program. See the derive section of the manual for how to use this.


In rare cases it may be necessary to implement `Serialize` manually for some
type in your program. See the Implementing `Serialize` section of the
manual for more about this.


Third-party crates may provide `Serialize` implementations for types that
they expose. For example the `linked-hash-map` crate provides a
`LinkedHashMap<K, V>` type that is serializable by Serde because the crate
provides an implementation of `Serialize` for it.

Source: <https://docs.rs/serde/1.0.193/serde/trait.Serialize.html>
