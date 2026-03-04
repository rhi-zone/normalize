#![allow(warnings, clippy::all)]
// Simplified lang_globs — builds file type filters from normalize-languages.
// Replaces upstream's lang_globs.rs which uses ast-grep-language's hardcoded extensions.

use super::Lang;
use ignore::types::{Types, TypesBuilder};
use std::collections::HashMap;

pub type LanguageGlobs = HashMap<String, Vec<String>>;

/// Build file type filters for a single language.
pub fn file_types_for_lang(lang: &Lang) -> Types {
    let mut builder = TypesBuilder::new();
    let name = lang.name();

    // Get extensions from normalize-languages registry
    if let Some(lang_support) = normalize_languages::support_for_grammar(name) {
        for ext in lang_support.extensions() {
            let glob = format!("*.{ext}");
            builder.add(name, &glob).unwrap_or_default();
        }
    }

    // Always select this lang name
    if builder.definitions().iter().any(|d| d.name() == name) {
        builder.select(name);
    }

    builder.build().unwrap_or_else(|_| {
        // Fallback: empty types (matches nothing)
        TypesBuilder::new().build().unwrap()
    })
}

/// Merge multiple Types into one.
pub fn merge_types(types_vec: impl Iterator<Item = Types>) -> Types {
    let mut builder = TypesBuilder::new();
    for types in types_vec {
        for def in types.definitions() {
            let name = def.name();
            for glob in def.globs() {
                builder.add(name, glob).expect(name);
            }
            builder.select(name);
        }
    }
    builder.build().expect("file types must be valid")
}
