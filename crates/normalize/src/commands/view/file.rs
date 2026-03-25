//! File skeleton viewing for view command.

use super::report::ViewReport;
use crate::skeleton::ExtractResultExt;
use crate::tree::{DocstringDisplay, ViewNode, ViewNodeKind};
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
) -> Result<ViewReport, String> {
    let full_path = root.join(file_path);
    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Error reading {}: {}", file_path, e))?;

    if !(0..=2).contains(&depth) {
        // depth < 0 or > 2: emit raw source
        let grammar = support_for_path(&full_path).map(|s| s.grammar_name().to_string());
        let line_count = content.lines().count();
        let mut node = ViewNode::file(
            Path::new(file_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file_path.to_string()),
            file_path,
        );
        node.line_range = Some((1, line_count));
        return Ok(ViewReport {
            target: file_path.to_string(),
            node,
            source: Some(content),
            imports: Vec::new(),
            exports: Vec::new(),
            parent_signatures: Vec::new(),
            line_range: None,
            grammar,
            warnings: Vec::new(),
            summary: None,
        });
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
        Some(deps::extract_deps(&full_path, &content))
    } else {
        None
    };

    let mut view_node = skeleton_result.to_view_node(grammar.as_deref());
    let line_count = content.lines().count();
    // Store line_count in the file node's line_range so the renderer can display "Lines: N"
    view_node.line_range = Some((1, line_count));

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

    // Ensure the node kind is File (it should be from to_view_node)
    debug_assert!(matches!(view_node.kind, ViewNodeKind::File));

    Ok(ViewReport {
        target: file_path.to_string(),
        node: view_node,
        source: None,
        imports,
        exports,
        parent_signatures: Vec::new(),
        line_range: None,
        grammar,
        warnings,
        summary: None,
    })
}
