# normalize-core/src

Source for the `normalize-core` crate.

`lib.rs` declares the `merge` module and re-exports `Merge` (trait) and `normalize_derive::Merge` (proc macro). `merge.rs` implements the `Merge` trait for all standard types: primitives (other always wins), `Option<T>` (merges inner values when both `Some`), `Vec` (other replaces self), `HashMap`/`BTreeMap` (other's keys override), and `HashSet`/`BTreeSet` (union).
