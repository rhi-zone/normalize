// A lib.rs without any inner doc comments — should trigger rust/missing-module-doc.

pub struct Foo;

impl Foo {
    pub fn bar(&self) {}
}
