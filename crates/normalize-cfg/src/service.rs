//! CLI service for the CFG command.
//!
//! Implements `normalize cfg` via the server-less `#[cli]` pattern.

use normalize_output::OutputFormatter;
use serde::Serialize;
use server_less::cli;
use streaming_iterator::StreamingIterator;

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Result of `normalize cfg`: the rendered CFG for a function.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CfgReport {
    /// Source file path.
    pub file: String,
    /// Function name (or filter used).
    pub function: Option<String>,
    /// Output format (e.g. `mermaid`).
    pub format: String,
    /// Rendered output (e.g. Mermaid flowchart text).
    pub output: String,
    /// Number of blocks in the CFG.
    pub block_count: usize,
    /// Number of edges in the CFG.
    pub edge_count: usize,
}

impl OutputFormatter for CfgReport {
    fn format_text(&self) -> String {
        self.output.clone()
    }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// CLI service implementing `normalize cfg`.
pub struct CfgService;

impl CfgService {
    /// Create a new `CfgService`.
    pub fn new() -> Self {
        Self
    }

    fn display_cfg(&self, r: &CfgReport) -> String {
        r.format_text()
    }
}

impl Default for CfgService {
    fn default() -> Self {
        Self::new()
    }
}

#[cli(
    name = "cfg",
    description = "Build and render the CFG for a function.\nAlso known as: control-flow graph, flow chart, code flow."
)]
impl CfgService {
    /// Build and render the control flow graph for a function in a source file.
    #[cli(display_with = "display_cfg")]
    pub fn cfg(
        &self,
        #[param(positional, help = "Source file path")] path: String,
        #[param(
            short = 'f',
            help = "Function name filter (defaults to first function found)"
        )]
        function: Option<String>,
        #[param(help = "Output format: mermaid (default)")] format: Option<String>,
    ) -> Result<CfgReport, String> {
        let format = format.unwrap_or_else(|| "mermaid".to_string());

        // Read source file.
        let source_bytes =
            std::fs::read(&path).map_err(|e| format!("failed to read {path}: {e}"))?;

        // Detect language via normalize-languages.
        let lang_support = normalize_languages::support_for_path(std::path::Path::new(&path))
            .ok_or_else(|| format!("no language support for file: {path}"))?;

        let grammar_name = lang_support.grammar_name();

        // Load grammar.
        let loader = normalize_languages::parsers::grammar_loader();
        let ts_language = loader
            .get(grammar_name)
            .map_err(|e| format!("failed to load grammar '{grammar_name}': {e}"))?;

        // Load CFG query.
        let cfg_query = loader
            .get_cfg(grammar_name)
            .ok_or_else(|| format!("no CFG query for language '{grammar_name}'"))?;

        // Parse source.
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&ts_language)
            .map_err(|e| format!("failed to set language: {e}"))?;
        let tree = parser
            .parse(&source_bytes, None)
            .ok_or_else(|| "failed to parse source".to_string())?;

        // Find function(s) via tags query.
        let tags_query_src = loader.get_tags(grammar_name).ok_or_else(|| {
            format!("no tags query for language '{grammar_name}' (needed to locate functions)")
        })?;

        let found_function = find_function_body(
            &tree,
            &tags_query_src,
            &source_bytes,
            grammar_name,
            function.as_deref(),
        )?;

        let (func_name, body_range, start_line) = found_function;

        let function_id = crate::FunctionId {
            file: path.clone(),
            qualified_name: func_name.clone(),
            start_line,
        };

        let cfg = crate::builder::build(&tree, &cfg_query, &source_bytes, function_id, body_range)
            .map_err(|e| format!("CFG build failed: {e}"))?;

        let output = match format.as_str() {
            "mermaid" => cfg.to_mermaid(),
            other => return Err(format!("unknown format '{other}'; supported: mermaid")),
        };

        Ok(CfgReport {
            file: path,
            function: Some(func_name),
            format,
            block_count: cfg.blocks.len(),
            edge_count: cfg.edges.len(),
            output,
        })
    }
}

// ---------------------------------------------------------------------------
// Function finding
// ---------------------------------------------------------------------------

/// Find a function body by name (or the first function if none specified).
/// Returns (name, body_byte_range, start_line_1based).
fn find_function_body(
    tree: &tree_sitter::Tree,
    tags_query_src: &str,
    source: &[u8],
    _grammar_name: &str,
    filter: Option<&str>,
) -> Result<(String, std::ops::Range<usize>, u32), String> {
    let language = tree.language();
    let query = tree_sitter::Query::new(&language, tags_query_src)
        .map_err(|e| format!("failed to compile tags query: {e}"))?;

    let capture_names = query.capture_names().to_vec();

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches_iter = cursor.matches(&query, tree.root_node(), source);

    // Look for definition.function or definition.method captures.
    // Collect raw data first to avoid borrow issues.
    struct CandidateRaw {
        func_name: String,
        def_start: usize,
        def_end: usize,
        start_line: u32,
    }
    let mut raw_candidates: Vec<CandidateRaw> = Vec::new();

    while let Some(mat) = matches_iter.next() {
        for cap in mat.captures {
            let name = capture_names[cap.index as usize];
            if name.starts_with("name.definition.function")
                || name.starts_with("name.definition.method")
                || name == "name.definition"
            {
                let func_name = cap
                    .node
                    .utf8_text(source)
                    .unwrap_or("<unknown>")
                    .to_string();

                let def_node = if let Some(p) = cap.node.parent() {
                    p
                } else {
                    cap.node
                };
                let start_line = def_node.start_position().row as u32 + 1;
                raw_candidates.push(CandidateRaw {
                    func_name,
                    def_start: def_node.start_byte(),
                    def_end: def_node.end_byte(),
                    start_line,
                });
            }
        }
    }
    drop(matches_iter);

    let mut candidates: Vec<(String, std::ops::Range<usize>, u32)> = Vec::new();
    for rc in raw_candidates {
        if filter.is_some_and(|f| rc.func_name != f && !rc.func_name.contains(f)) {
            continue;
        }
        candidates.push((rc.func_name, rc.def_start..rc.def_end, rc.start_line));
    }

    if candidates.is_empty() {
        // Fallback: use the entire file as body.
        let root = tree.root_node();
        return Ok((
            filter.unwrap_or("<file>").to_string(),
            root.start_byte()..root.end_byte(),
            1,
        ));
    }

    Ok(candidates.into_iter().next().unwrap())
}
