//! XML language support.

use crate::Language;
use tree_sitter::Node;

/// XML language support.
pub struct Xml;

impl Language for Xml {
    fn name(&self) -> &'static str {
        "XML"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["xml", "xsl", "xslt", "xsd", "svg", "plist"]
    }
    fn grammar_name(&self) -> &'static str {
        "xml"
    }

    fn has_symbols(&self) -> bool {
        false
    }

    fn node_name<'a>(&self, _node: &Node, _content: &'a str) -> Option<&'a str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "Enumeration", "NotationType", "StringType", "TokenizedType",
            "doctypedecl",
        ];
        validate_unused_kinds_audit(&Xml, documented_unused)
            .expect("XML unused node kinds audit failed");
    }
}
