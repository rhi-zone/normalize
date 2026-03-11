//! File skeleton viewing for view command.

use super::report::{ViewFileContentReport, ViewFileReport, ViewOutput};
use crate::skeleton::ExtractResultExt;
use crate::tree::DocstringDisplay;
use crate::{deps, skeleton};
use normalize_languages::support_for_path;
use std::path::Path;

/// Build file skeleton view for the service layer.
#[allow(clippy::too_many_arguments)]
pub fn build_view_file_service(
    file_path: &str,
    root: &Path,
    depth: i32,
    show_deps: bool,
    types_only: bool,
    show_tests: bool,
    _docstring_mode: DocstringDisplay,
    context: bool,
) -> Result<ViewOutput, String> {
    let full_path = root.join(file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Error reading {}: {}", file_path, e))?;

    if !(0..=2).contains(&depth) {
        let grammar = support_for_path(&full_path).map(|s| s.grammar_name().to_string());
        return Ok(ViewOutput::FileContent(ViewFileContentReport {
            path: file_path.to_string(),
            content,
            grammar,
        }));
    }

    let support = support_for_path(&full_path);
    let grammar = support.as_ref().map(|s| s.grammar_name().to_string());

    let mut warnings = Vec::new();
    if let Some(lang) = support
        && lang.as_symbols().is_none()
    {
        warnings.push(format!(
            "{} is a data/config language — symbol extraction is not supported",
            lang.name()
        ));
    }

    let extractor = skeleton::SkeletonExtractor::new();
    let skeleton_result = extractor.extract(&full_path, &content);

    let skeleton_result = if types_only {
        skeleton_result.filter_types()
    } else if !show_tests {
        skeleton_result.filter_tests()
    } else {
        skeleton_result
    };

    let deps_result = if show_deps || context {
        let deps_extractor = deps::DepsExtractor::new();
        Some(deps_extractor.extract(&full_path, &content))
    } else {
        None
    };

    let view_node = skeleton_result.to_view_node(grammar.as_deref());
    let line_count = content.lines().count();

    let mut imports = Vec::new();
    let mut exports = Vec::new();

    if let Some(ref deps) = deps_result {
        for imp in &deps.imports {
            if imp.names.is_empty() {
                imports.push(format!("  import {}", imp.module));
            } else {
                imports.push(format!(
                    "  from {} import {}",
                    imp.module,
                    imp.names.join(", ")
                ));
            }
        }
        for exp in &deps.exports {
            exports.push(format!("  {}", exp.name));
        }
    }

    Ok(ViewOutput::File(ViewFileReport {
        path: file_path.to_string(),
        line_count,
        imports,
        exports,
        node: view_node,
        warnings,
    }))
}
