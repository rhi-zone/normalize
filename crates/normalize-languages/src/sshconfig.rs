//! SSH config file support.

use crate::{Language, LanguageSymbols};

/// SSH config language support.
pub struct SshConfig;

impl Language for SshConfig {
    fn name(&self) -> &'static str {
        "SSH Config"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    } // Matched by filename
    fn grammar_name(&self) -> &'static str {
        "ssh-config"
    }

    fn as_symbols(&self) -> Option<&dyn LanguageSymbols> {
        Some(self)
    }
}

impl LanguageSymbols for SshConfig {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_unused_kinds_audit;

    #[test]
    fn unused_node_kinds_audit() {
        #[rustfmt::skip]
        let documented_unused: &[&str] = &[
            "host_declaration", "match_declaration",
        ];
        validate_unused_kinds_audit(&SshConfig, documented_unused)
            .expect("SSH Config unused node kinds audit failed");
    }
}
