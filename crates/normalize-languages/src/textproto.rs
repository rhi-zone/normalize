//! Protocol Buffers text format support.

use crate::{Language, LanguageSymbols};

/// TextProto language support.
pub struct TextProto;

impl Language for TextProto {
    fn name(&self) -> &'static str {
        "TextProto"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["textproto", "pbtxt"]
    }
    fn grammar_name(&self) -> &'static str {
        "textproto"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }
}

impl LanguageSymbols for TextProto {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "identifier", "type_name", "signed_identifier",
        ];
        validate_unused_kinds_audit(&TextProto, documented_unused)
            .expect("TextProto unused node kinds audit failed");
    }
}
