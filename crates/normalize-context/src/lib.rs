//! Frontmatter-filtered context resolution.
//!
//! Resolves contextual text from `.normalize/context/` directories walked
//! hierarchically (project → parent → ... → global `~/.normalize/context/`).
//! Source files are Markdown with optional YAML frontmatter. Frontmatter is
//! matched against caller-provided context to filter which blocks are returned.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Report structs
// ---------------------------------------------------------------------------

/// Context file list report (--list mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContextListReport {
    pub files: Vec<PathBuf>,
}

impl ContextListReport {
    pub fn new(files: Vec<PathBuf>) -> Self {
        Self { files }
    }
}

/// A single resolved context block (one frontmatter+body unit).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContextBlock {
    pub source: PathBuf,
    pub metadata: serde_json::Value,
    pub body: String,
}

/// Context report (default mode).
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ContextReport {
    pub blocks: Vec<ContextBlock>,
}

impl ContextReport {
    pub fn new(blocks: Vec<ContextBlock>) -> Self {
        Self { blocks }
    }
}

// ---------------------------------------------------------------------------
// OutputFormatter impls (CLI wiring, gated by `cli` feature)
// ---------------------------------------------------------------------------

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for ContextListReport {
    fn format_text(&self) -> String {
        if self.files.is_empty() {
            "No context files found.".to_string()
        } else {
            self.files
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

#[cfg(feature = "cli")]
impl normalize_output::OutputFormatter for ContextReport {
    fn format_text(&self) -> String {
        if self.blocks.is_empty() {
            return "No context found.".to_string();
        }
        if self.blocks.len() == 1 {
            return self.blocks[0].body.trim().to_string();
        }
        let mut out = String::new();
        for block in &self.blocks {
            if !out.is_empty() {
                out.push_str("\n\n---\n\n");
            }
            out.push_str(&format!("<!-- {} -->\n\n", block.source.display()));
            out.push_str(block.body.trim());
        }
        out
    }

    fn format_pretty(&self) -> String {
        if self.blocks.is_empty() {
            return "No context found.".to_string();
        }
        let mut out = String::new();
        for block in &self.blocks {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("<!-- {} -->\n", block.source.display()));
            let body = block.body.trim();
            if !body.is_empty() {
                out.push('\n');
                out.push_str(body);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Caller context type
// ---------------------------------------------------------------------------

/// A flat key-value map representing caller-provided context.
/// Keys are dot-paths (e.g. `"claudecode.hook"`), values are strings.
pub type CallerContext = HashMap<String, String>;

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

/// A parsed block: optional frontmatter YAML value + body text.
#[derive(Debug)]
pub struct ParsedBlock {
    pub frontmatter: Option<serde_yaml::Value>,
    pub body: String,
}

/// Parse a Markdown file into one or more blocks.
///
/// A file may contain multiple blocks separated by `---` fence lines. Each block is:
/// - `---\n<yaml>\n---\n<body>` — a block with frontmatter
/// - plain text — a block without frontmatter (always matches)
///
/// The file may begin with `---` (standard frontmatter) or directly with content.
/// Multiple blocks per file are supported: after a body, a new `---` starts the
/// next block's YAML frontmatter.
pub fn parse_blocks(content: &str) -> Vec<ParsedBlock> {
    // Collect positions of `---` fence lines (lines that are exactly `---`).
    let lines: Vec<&str> = content.lines().collect();
    let mut fence_positions: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| **l == "---")
        .map(|(i, _)| i)
        .collect();

    if fence_positions.is_empty() {
        // No fences → single plain body block.
        return vec![ParsedBlock {
            frontmatter: None,
            body: content.to_string(),
        }];
    }

    let mut blocks = Vec::new();
    let total_lines = lines.len();

    // If first fence is at line 0, the file starts with frontmatter.
    // We process fences in pairs: fence[n] opens YAML, fence[n+1] closes it.
    // Between two fences is YAML; after the closing fence is the body until the
    // next opening fence (or EOF).
    //
    // If there's content before the first fence, it's a plain body block.

    // Consume any content before the first fence as a plain block.
    if fence_positions[0] > 0 {
        let body = lines[..fence_positions[0]].join("\n");
        if !body.trim().is_empty() {
            blocks.push(ParsedBlock {
                frontmatter: None,
                body,
            });
        }
        let start = fence_positions[0];
        fence_positions.retain(|&f| f >= start);
    }

    // Now process fence pairs: opening fence at fence_positions[0], closing at fence_positions[1].
    let mut fp_idx = 0usize;
    while fp_idx < fence_positions.len() {
        let open_fence = fence_positions[fp_idx];

        if fp_idx + 1 < fence_positions.len() {
            // We have a closing fence.
            let close_fence = fence_positions[fp_idx + 1];

            // YAML content is between open_fence+1 and close_fence-1.
            let yaml_lines = &lines[open_fence + 1..close_fence];
            let yaml_text = yaml_lines.join("\n");

            // Try to parse as YAML mapping.
            let frontmatter = match serde_yaml::from_str::<serde_yaml::Value>(&yaml_text) {
                Ok(v) if v.is_mapping() || v.is_null() => Some(v),
                _ => None,
            };

            // Body is from close_fence+1 until the next open fence (or EOF).
            let body_start = close_fence + 1;
            let body_end = if fp_idx + 2 < fence_positions.len() {
                fence_positions[fp_idx + 2]
            } else {
                total_lines
            };
            let body = lines[body_start..body_end].join("\n");

            if frontmatter.is_some() {
                blocks.push(ParsedBlock { frontmatter, body });
                fp_idx += 2;
            } else {
                // Didn't parse as YAML → treat the whole region as a plain body.
                let plain_body = lines[open_fence..body_end].join("\n");
                blocks.push(ParsedBlock {
                    frontmatter: None,
                    body: plain_body,
                });
                fp_idx += 2;
            }
        } else {
            // Orphan opening fence with no closing fence — treat remainder as plain body.
            let body = lines[open_fence..].join("\n");
            blocks.push(ParsedBlock {
                frontmatter: None,
                body,
            });
            fp_idx += 1;
        }
    }

    blocks
}

// ---------------------------------------------------------------------------
// Matching logic
// ---------------------------------------------------------------------------

/// Look up a dot-path in a `serde_yaml::Value`.
/// `"claudecode.hook"` navigates `{claudecode: {hook: ...}}`.
#[allow(dead_code)]
fn yaml_get_dotpath<'a>(value: &'a serde_yaml::Value, path: &str) -> Option<&'a serde_yaml::Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// Look up a dot-path key in the caller context.
fn caller_get(ctx: &CallerContext, path: &str) -> Option<String> {
    ctx.get(path).cloned()
}

/// Convert a `serde_yaml::Value` scalar to a string for comparison.
fn yaml_scalar_to_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Null => Some("null".to_string()),
        _ => None,
    }
}

/// Evaluate a single match strategy leaf.
///
/// `field_path` — dot-path into the caller context
/// `strategy_node` — YAML value describing the strategy (`{equals: "..."}` etc.)
/// `caller_ctx` — caller-provided context
pub fn eval_strategy(
    field_path: &str,
    strategy_node: &serde_yaml::Value,
    caller_ctx: &CallerContext,
) -> bool {
    match strategy_node {
        // Bare scalar → shorthand for equals.
        serde_yaml::Value::String(_)
        | serde_yaml::Value::Bool(_)
        | serde_yaml::Value::Number(_) => {
            let expected = yaml_scalar_to_string(strategy_node).unwrap_or_default();
            caller_get(caller_ctx, field_path)
                .map(|v| v == expected)
                .unwrap_or(false)
        }
        serde_yaml::Value::Mapping(map) => {
            // Expect exactly one key: the strategy name.
            let (strategy_name, args) = match map.iter().next() {
                Some(pair) => pair,
                None => return false,
            };
            let strategy_name = match strategy_name.as_str() {
                Some(s) => s,
                None => return false,
            };
            match strategy_name {
                "equals" => {
                    let expected = yaml_scalar_to_string(args).unwrap_or_default();
                    caller_get(caller_ctx, field_path)
                        .map(|v| v == expected)
                        .unwrap_or(false)
                }
                "contains" => {
                    let needle = yaml_scalar_to_string(args).unwrap_or_default();
                    caller_get(caller_ctx, field_path)
                        .map(|v| v.contains(needle.as_str()))
                        .unwrap_or(false)
                }
                "keywords" => {
                    let keywords: Vec<String> = match args {
                        serde_yaml::Value::Sequence(seq) => {
                            seq.iter().filter_map(yaml_scalar_to_string).collect()
                        }
                        _ => return false,
                    };
                    let caller_val = match caller_get(caller_ctx, field_path) {
                        Some(v) => v,
                        None => return false,
                    };
                    keywords.iter().any(|kw| caller_val.contains(kw.as_str()))
                }
                "regex" => {
                    let pattern = yaml_scalar_to_string(args).unwrap_or_default();
                    let re = match regex::Regex::new(&pattern) {
                        Ok(r) => r,
                        Err(_) => return false,
                    };
                    caller_get(caller_ctx, field_path)
                        .map(|v| re.is_match(&v))
                        .unwrap_or(false)
                }
                "exists" => {
                    let expected_exists = match args {
                        serde_yaml::Value::Bool(b) => *b,
                        _ => true,
                    };
                    let actual_exists = caller_get(caller_ctx, field_path).is_some();
                    actual_exists == expected_exists
                }
                "one_of" => {
                    let options: Vec<String> = match args {
                        serde_yaml::Value::Sequence(seq) => {
                            seq.iter().filter_map(yaml_scalar_to_string).collect()
                        }
                        _ => return false,
                    };
                    caller_get(caller_ctx, field_path)
                        .map(|v| options.iter().any(|opt| opt == &v))
                        .unwrap_or(false)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Evaluate a conditions node (may contain `all:` / `any:` / leaf conditions).
pub fn eval_conditions(node: &serde_yaml::Value, caller_ctx: &CallerContext) -> bool {
    match node {
        serde_yaml::Value::Mapping(map) => {
            // Check for `all:` / `any:` keys.
            let has_all = map.contains_key("all");
            let has_any = map.contains_key("any");

            if has_all || has_any {
                let mut result = true;

                if has_all && let Some(serde_yaml::Value::Sequence(items)) = map.get("all") {
                    for item in items {
                        if !eval_conditions(item, caller_ctx) {
                            result = false;
                            break;
                        }
                    }
                }

                if has_any && let Some(serde_yaml::Value::Sequence(items)) = map.get("any") {
                    let any_pass = items.iter().any(|item| eval_conditions(item, caller_ctx));
                    result = result && any_pass;
                }

                result
            } else {
                // Leaf: { field.path: {strategy: args} } or { field.path: bare_value }
                // All entries in the map must pass (AND).
                for (field_key, strategy_node) in map {
                    let field_path = match field_key.as_str() {
                        Some(s) => s,
                        None => return false,
                    };
                    if !eval_strategy(field_path, strategy_node, caller_ctx) {
                        return false;
                    }
                }
                true
            }
        }
        serde_yaml::Value::Sequence(items) => {
            // Bare sequence → treat as `all:`.
            items.iter().all(|item| eval_conditions(item, caller_ctx))
        }
        _ => false,
    }
}

/// Decide whether a parsed block matches the caller context.
///
/// Rules:
/// 1. `--all` → always true (caller passes `all_flag=true`).
/// 2. Block has `conditions:` → evaluate the conditions tree.
/// 3. No `conditions:`: for each frontmatter key (excluding `conditions:`), try
///    dot-path lookup in caller context with `equals`. Missing caller keys don't
///    fail. If frontmatter is empty or null → always matches.
pub fn block_matches(block: &ParsedBlock, caller_ctx: &CallerContext, all_flag: bool) -> bool {
    if all_flag {
        return true;
    }

    let fm = match &block.frontmatter {
        None => return true, // No frontmatter → always matches.
        Some(v) => v,
    };

    // Null / empty frontmatter → always matches.
    if fm.is_null() {
        return true;
    }
    let map = match fm.as_mapping() {
        Some(m) => m,
        None => return true,
    };
    if map.is_empty() {
        return true;
    }

    // Check for `conditions:` key.
    if let Some(conditions_node) = map.get("conditions") {
        return eval_conditions(conditions_node, caller_ctx);
    }

    // No conditions block: match each frontmatter key with equals.
    // Only keys that also exist in caller_ctx are compared (missing = skip, not fail).
    // If no keys overlap → always matches (caller context irrelevant).
    let mut any_key_compared = false;
    for (k, v) in map {
        let key_str = match k.as_str() {
            Some(s) => s,
            None => continue,
        };
        // Navigate nested keys: `claudecode.hook` → check if caller has that dot-path.
        // We need to flatten the YAML frontmatter key hierarchy to dot-paths for comparison.
        // E.g., frontmatter `claudecode: {hook: UserPromptSubmit}` → key `claudecode`, value mapping.
        // Compare by flattening the frontmatter value against caller context.
        if !compare_yaml_to_caller(key_str, v, caller_ctx, &mut any_key_compared) {
            return false;
        }
    }

    true
}

/// Recursively compare a frontmatter subtree against the caller context.
/// Returns `false` if any present caller key fails its equals check.
/// Sets `any_key_compared` to `true` if any key was compared.
fn compare_yaml_to_caller(
    prefix: &str,
    value: &serde_yaml::Value,
    caller_ctx: &CallerContext,
    _any_compared: &mut bool,
) -> bool {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                let child_key = match k.as_str() {
                    Some(s) => s,
                    None => continue,
                };
                let full_path = format!("{prefix}.{child_key}");
                if !compare_yaml_to_caller(&full_path, v, caller_ctx, _any_compared) {
                    return false;
                }
            }
            true
        }
        scalar => {
            // Leaf: compare against caller context at `prefix`.
            if let Some(caller_val) = caller_get(caller_ctx, prefix) {
                *_any_compared = true;
                let fm_val = yaml_scalar_to_string(scalar).unwrap_or_default();
                caller_val == fm_val
            } else {
                // Key not in caller context → skip (don't fail).
                true
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Directory walk
// ---------------------------------------------------------------------------

/// Collect `.md` files from `.normalize/{dir_name}/` directories walked
/// bottom-up from `start_dir` toward the filesystem root.
/// Also includes `~/.normalize/{dir_name}/` as a global layer.
///
/// Returns files in order: closest (project-specific) first, global last.
pub fn collect_new_context_files(start_dir: &Path, dir_name: &str) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut current = start_dir.to_path_buf();
    let home = dirs::home_dir();

    // Walk up from start_dir, stopping before home (home is added explicitly below).
    loop {
        // Stop the walk at home — the explicit global append below covers it,
        // avoiding duplicates when start_dir is inside the home directory.
        if home.as_deref() == Some(current.as_path()) {
            break;
        }

        let ctx_dir = current.join(".normalize").join(dir_name);
        if ctx_dir.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(&ctx_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
                .collect();
            entries.sort();
            result.extend(entries);
        }

        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }

    // Global layer: ~/.normalize/{dir_name}/
    if let Some(home) = home {
        let global_dir = home.join(".normalize").join(dir_name);
        if global_dir.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(&global_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
                .collect();
            entries.sort();
            result.extend(entries);
        }
    }

    result
}

/// Resolve context blocks from the hierarchy.
///
/// Files are collected bottom-up (project-specific first, global last).
/// Blocks within each file are parsed and matched against `caller_ctx`.
pub fn resolve_context(
    start_dir: &Path,
    dir_name: &str,
    caller_ctx: &CallerContext,
    all_flag: bool,
) -> Vec<(PathBuf, serde_yaml::Value, String)> {
    let files = collect_new_context_files(start_dir, dir_name);
    let mut results = Vec::new();

    for file_path in &files {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let blocks = parse_blocks(&content);
        for block in blocks {
            if block_matches(&block, caller_ctx, all_flag) {
                let body = block.body.trim().to_string();
                if body.is_empty() && block.frontmatter.is_none() {
                    continue;
                }
                let metadata = block.frontmatter.unwrap_or(serde_yaml::Value::Null);
                results.push((file_path.clone(), metadata, body));
            }
        }
    }

    results
}

/// Convert a `serde_yaml::Value` to `serde_json::Value`.
pub fn yaml_to_json(v: serde_yaml::Value) -> serde_json::Value {
    // serde_yaml → serde_json via intermediate serialization.
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_parse_blocks_plain() {
        let blocks = parse_blocks("Hello world");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].frontmatter.is_none());
        assert_eq!(blocks[0].body.trim(), "Hello world");
    }

    #[test]
    fn test_parse_blocks_with_frontmatter() {
        let content = "---\nhook: test\n---\nBody text\n";
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].frontmatter.is_some());
        assert_eq!(blocks[0].body.trim(), "Body text");
    }

    #[test]
    fn test_parse_blocks_multiple() {
        let content = "---\nhook: a\n---\nBlock A\n\n---\nhook: b\n---\nBlock B\n";
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].body.trim(), "Block A");
        assert_eq!(blocks[1].body.trim(), "Block B");
    }

    #[test]
    fn test_block_matches_no_frontmatter() {
        let block = ParsedBlock {
            frontmatter: None,
            body: "text".into(),
        };
        let ctx = CallerContext::new();
        assert!(block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_block_matches_empty_frontmatter() {
        let block = ParsedBlock {
            frontmatter: Some(serde_yaml::Value::Mapping(Default::default())),
            body: "text".into(),
        };
        let ctx = CallerContext::new();
        assert!(block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_block_matches_simple_equals_pass() {
        let fm: serde_yaml::Value = serde_yaml::from_str("hook: UserPromptSubmit").unwrap();
        let block = ParsedBlock {
            frontmatter: Some(fm),
            body: "text".into(),
        };
        let mut ctx = CallerContext::new();
        ctx.insert("hook".into(), "UserPromptSubmit".into());
        assert!(block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_block_matches_simple_equals_fail() {
        let fm: serde_yaml::Value = serde_yaml::from_str("hook: UserPromptSubmit").unwrap();
        let block = ParsedBlock {
            frontmatter: Some(fm),
            body: "text".into(),
        };
        let mut ctx = CallerContext::new();
        ctx.insert("hook".into(), "OtherHook".into());
        assert!(!block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_block_matches_missing_caller_key_passes() {
        // Frontmatter has `hook: X` but caller doesn't provide `hook` → matches (skip).
        let fm: serde_yaml::Value = serde_yaml::from_str("hook: UserPromptSubmit").unwrap();
        let block = ParsedBlock {
            frontmatter: Some(fm),
            body: "text".into(),
        };
        let ctx = CallerContext::new(); // no keys
        assert!(block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_block_matches_nested_frontmatter() {
        let fm: serde_yaml::Value =
            serde_yaml::from_str("claudecode:\n  hook: UserPromptSubmit\n").unwrap();
        let block = ParsedBlock {
            frontmatter: Some(fm),
            body: "text".into(),
        };
        let mut ctx = CallerContext::new();
        ctx.insert("claudecode.hook".into(), "UserPromptSubmit".into());
        assert!(block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_block_matches_all_flag() {
        let fm: serde_yaml::Value = serde_yaml::from_str("hook: something").unwrap();
        let block = ParsedBlock {
            frontmatter: Some(fm),
            body: "text".into(),
        };
        let ctx = CallerContext::new();
        assert!(block_matches(&block, &ctx, true));
    }

    #[test]
    fn test_conditions_equals() {
        let fm: serde_yaml::Value = serde_yaml::from_str(
            "conditions:\n  all:\n    - hook:\n        equals: UserPromptSubmit\n",
        )
        .unwrap();
        let block = ParsedBlock {
            frontmatter: Some(fm),
            body: "text".into(),
        };
        let mut ctx = CallerContext::new();
        ctx.insert("hook".into(), "UserPromptSubmit".into());
        assert!(block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_conditions_any() {
        let fm: serde_yaml::Value = serde_yaml::from_str(
            "conditions:\n  any:\n    - lang:\n        equals: rust\n    - lang:\n        equals: go\n",
        )
        .unwrap();
        let block = ParsedBlock {
            frontmatter: Some(fm),
            body: "text".into(),
        };
        let mut ctx = CallerContext::new();
        ctx.insert("lang".into(), "go".into());
        assert!(block_matches(&block, &ctx, false));
    }

    #[test]
    fn test_resolve_context_basic() {
        let tmp = tempdir().unwrap();
        let ctx_dir = tmp.path().join(".normalize").join("context");
        fs::create_dir_all(&ctx_dir).unwrap();
        fs::write(
            ctx_dir.join("hints.md"),
            "---\nhook: test\n---\nDo the thing.\n",
        )
        .unwrap();

        let mut caller = CallerContext::new();
        caller.insert("hook".into(), "test".into());

        let results = resolve_context(tmp.path(), "context", &caller, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "Do the thing.");
    }
}
