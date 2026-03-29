//! Rule execution with combined query optimization.

use crate::sources::{SourceContext, SourceRegistry, builtin_registry};
use crate::{Rule, Severity};
use normalize_languages::{GrammarLoader, support_for_path};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

/// Serializable representation of a `Finding` for caching.
///
/// Same fields as `Finding` but derives `Serialize`/`Deserialize` so it can be
/// persisted to `.normalize/syntax-cache.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedFinding {
    pub rule_id: String,
    pub file: std::path::PathBuf,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub message: String,
    pub severity: Severity,
    pub matched_text: String,
    pub fix: Option<String>,
    pub captures: HashMap<String, String>,
}

impl From<Finding> for CachedFinding {
    fn from(f: Finding) -> Self {
        Self {
            rule_id: f.rule_id,
            file: f.file,
            start_line: f.start_line,
            start_col: f.start_col,
            end_line: f.end_line,
            end_col: f.end_col,
            start_byte: f.start_byte,
            end_byte: f.end_byte,
            message: f.message,
            severity: f.severity,
            matched_text: f.matched_text,
            fix: f.fix,
            captures: f.captures,
        }
    }
}

impl From<CachedFinding> for Finding {
    fn from(c: CachedFinding) -> Self {
        Self {
            rule_id: c.rule_id,
            file: c.file,
            start_line: c.start_line,
            start_col: c.start_col,
            end_line: c.end_line,
            end_col: c.end_col,
            start_byte: c.start_byte,
            end_byte: c.end_byte,
            message: c.message,
            severity: c.severity,
            matched_text: c.matched_text,
            fix: c.fix,
            captures: c.captures,
        }
    }
}

/// Per-file cache entry: mtime + findings from the last successful run.
#[derive(serde::Serialize, serde::Deserialize)]
struct FileCacheEntry {
    /// Nanoseconds since UNIX epoch (subsecond precision avoids false hits within the same second).
    mtime_nanos: u128,
    findings: Vec<CachedFinding>,
}

/// On-disk cache for syntax-rule findings, keyed by file path.
///
/// Stored at `.normalize/syntax-cache.json` inside the project root.
/// Invalidated in full when the active rule set changes.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SyntaxCache {
    /// Hash of the active rule IDs + their query source, used for global invalidation.
    rules_hash: String,
    files: HashMap<String, FileCacheEntry>,
}

impl SyntaxCache {
    fn load(project_root: &Path) -> Self {
        let path = project_root.join(".normalize/syntax-cache.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self, project_root: &Path) {
        let dir = project_root.join(".normalize");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("syntax-cache.json");
        if let Ok(json) = serde_json::to_string(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// Compute a hash of the active rule set for cache invalidation.
fn compute_rules_hash(rules: &[&Rule]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    for rule in rules {
        rule.id.hash(&mut hasher);
        rule.query_str.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

/// Get the mtime of a file in nanoseconds since UNIX epoch, or 0 on failure.
///
/// Nanosecond precision is used to avoid false cache hits when a file is
/// modified within the same second (e.g. during multi-pass fix application in
/// tests and CI).
fn file_mtime_nanos(path: &Path) -> u128 {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// A finding from running a rule.
#[derive(Debug, Clone)]
pub struct Finding {
    /// ID of the rule that produced this finding.
    pub rule_id: String,
    /// Absolute path to the file where the finding was detected.
    pub file: PathBuf,
    /// 1-based line number of the start of the match.
    pub start_line: usize,
    /// 0-based column of the start of the match.
    pub start_col: usize,
    /// 1-based line number of the end of the match.
    pub end_line: usize,
    /// 0-based column of the end of the match.
    pub end_col: usize,
    /// Byte offset of the start of the match in the source file.
    pub start_byte: usize,
    /// Byte offset of the end of the match in the source file.
    pub end_byte: usize,
    /// Human-readable description of the finding.
    pub message: String,
    /// Severity level of the finding.
    pub severity: Severity,
    /// The source text of the matched node.
    pub matched_text: String,
    /// Auto-fix template (None if no fix available).
    pub fix: Option<String>,
    /// Capture values from the query match keyed by capture name (without `@`).
    /// Includes all named captures from the query; `@match` is NOT included —
    /// use `matched_text` for the full matched node text instead.
    pub captures: HashMap<String, String>,
}

/// Debug output categories.
#[derive(Default)]
pub struct DebugFlags {
    /// Whether to emit per-rule timing information to stderr.
    pub timing: bool,
}

impl DebugFlags {
    pub fn from_args(args: &[String]) -> Self {
        let all = args.iter().any(|s| s == "all");
        Self {
            timing: all || args.iter().any(|s| s == "timing"),
        }
    }
}

/// Check if a line contains a `normalize-syntax-allow:` comment for the given rule.
/// Supports: `// normalize-syntax-allow: rule-id` or `/* normalize-syntax-allow: rule-id */`
fn line_has_allow_comment(line: &str, rule_id: &str) -> bool {
    // Look for normalize-syntax-allow: followed by the rule ID
    // Pattern: normalize-syntax-allow: rule-id (optionally followed by - reason)
    if let Some(pos) = line.find("normalize-syntax-allow:") {
        let after = &line[pos + 23..]; // len("normalize-syntax-allow:")
        let after = after.trim_start();
        // Check if rule_id matches (might be followed by space, dash, or end of comment)
        if let Some(rest) = after.strip_prefix(rule_id) {
            // Valid if followed by nothing, whitespace, dash (reason), or end of comment
            return rest.is_empty()
                || rest.starts_with(char::is_whitespace)
                || rest.starts_with('-')
                || rest.starts_with("*/");
        }
    }
    false
}

/// Check if a finding should be allowed based on inline comments.
/// Checks the line of the finding and up to 2 lines before (to handle
/// multi-line expressions like `let x =\n    expr.unwrap()`).
fn is_allowed_by_comment(content: &str, start_line: usize, rule_id: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = start_line.saturating_sub(1); // 0-indexed

    for offset in 0..=2usize {
        let Some(idx) = line_idx.checked_sub(offset) else {
            break;
        };
        if let Some(line) = lines.get(idx)
            && line_has_allow_comment(line, rule_id)
        {
            return true;
        }
    }

    false
}

/// Check if a rule's requires conditions are met for a given file context.
///
/// Supports operators:
/// - `value` - exact match
/// - `>=value` - greater or equal (for versions/editions)
/// - `<=value` - less or equal
/// - `!value` - not equal
fn check_requires(rule: &Rule, registry: &SourceRegistry, ctx: &SourceContext) -> bool {
    if rule.requires.is_empty() {
        return true;
    }

    for (key, expected) in &rule.requires {
        let actual = match registry.get(ctx, key) {
            Some(v) => v,
            None => return false, // Required source not available
        };

        // Parse operator prefix
        let matches = if let Some(rest) = expected.strip_prefix(">=") {
            *actual >= *rest
        } else if let Some(rest) = expected.strip_prefix("<=") {
            *actual <= *rest
        } else if let Some(rest) = expected.strip_prefix('!') {
            actual != rest
        } else {
            actual == *expected
        };

        if !matches {
            return false;
        }
    }

    true
}

/// Combined query for a grammar with pattern-to-rule mapping.
struct CombinedQuery<'a> {
    query: tree_sitter::Query,
    /// Maps pattern_index to (rule, match_capture_index_in_combined_query)
    pattern_to_rule: Vec<(&'a Rule, usize)>,
}

/// Try to compile a cross-language rule for a grammar, falling back to
/// per-pattern compilation when the full query fails.
fn compile_cross_language_rule(
    rule: &Rule,
    grammar: &tree_sitter::Language,
) -> Option<(tree_sitter::Query, String)> {
    if let Ok(q) = tree_sitter::Query::new(grammar, &rule.query_str) {
        return Some((q, rule.query_str.clone()));
    }
    // Full query failed — try each pattern separately
    let patterns: Vec<&str> = split_query_patterns(&rule.query_str);
    if patterns.len() <= 1 {
        return None;
    }
    let valid: Vec<&str> = patterns
        .into_iter()
        .filter(|p| tree_sitter::Query::new(grammar, p).is_ok())
        .collect();
    if valid.is_empty() {
        return None;
    }
    let combined = valid.join("\n");
    tree_sitter::Query::new(grammar, &combined)
        .ok()
        .map(|q| (q, combined))
}

/// Compile per-grammar rules and build a combined query.
fn build_combined_query<'a>(
    grammar_name: &str,
    grammar: &tree_sitter::Language,
    specific_rules: &[&&'a Rule],
    global_rules: &[&&'a Rule],
) -> Option<CombinedQuery<'a>> {
    let mut compiled_rules: Vec<(&Rule, tree_sitter::Query, String)> = Vec::new();

    // Pass 1: Language-specific rules - compile directly (trust the author)
    for rule in specific_rules {
        if rule.languages.iter().any(|l| l == grammar_name)
            && let Ok(q) = tree_sitter::Query::new(grammar, &rule.query_str)
        {
            compiled_rules.push((rule, q, rule.query_str.clone()));
        }
    }

    // Pass 2: Cross-language rules - validate each one with pattern fallback
    for rule in global_rules {
        if let Some((q, qs)) = compile_cross_language_rule(rule, grammar) {
            compiled_rules.push((rule, q, qs));
        }
    }

    if compiled_rules.is_empty() {
        return None;
    }

    // Combine all into one query
    let combined_str = compiled_rules
        .iter()
        .map(|(_, _, qs)| qs.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let query = match tree_sitter::Query::new(grammar, &combined_str) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Warning: combined query failed for {}: {}", grammar_name, e);
            return None;
        }
    };

    // Map pattern indices to rules
    let combined_match_idx = query
        .capture_names()
        .iter()
        .position(|n| *n == "match")
        .unwrap_or(0);

    let mut pattern_to_rule: Vec<(&Rule, usize)> = Vec::new();
    for (rule, individual_query, _) in &compiled_rules {
        for _ in 0..individual_query.pattern_count() {
            pattern_to_rule.push((*rule, combined_match_idx));
        }
    }

    Some(CombinedQuery {
        query,
        pattern_to_rule,
    })
}

/// Build a Finding from a matched capture node.
fn build_finding(
    rule: &Rule,
    node: tree_sitter::Node,
    content: &str,
    query: &tree_sitter::Query,
    m: &tree_sitter::QueryMatch,
    file: &Path,
) -> Finding {
    let text = node.utf8_text(content.as_bytes()).unwrap_or("");

    let mut captures_map: HashMap<String, String> = HashMap::new();
    for cap in m.captures {
        let name = query.capture_names()[cap.index as usize].to_string();
        if let Ok(cap_text) = cap.node.utf8_text(content.as_bytes()) {
            captures_map.insert(name, cap_text.to_string());
        }
    }

    Finding {
        rule_id: rule.id.clone(),
        file: file.to_path_buf(),
        start_line: node.start_position().row + 1,
        start_col: node.start_position().column + 1,
        end_line: node.end_position().row + 1,
        end_col: node.end_position().column + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        message: rule.message.clone(),
        severity: rule.severity,
        matched_text: text.lines().next().unwrap_or("").to_string(),
        fix: rule.fix.clone(),
        captures: captures_map,
    }
}

/// Resolved allow-list path for a file.
struct AllowPath<'a> {
    /// Full path (if root_in_project was set).
    _full: Option<PathBuf>,
    /// String representation for allow-list matching.
    display: std::borrow::Cow<'a, str>,
}

/// Compute the allow-list path for a file (project-root-relative).
fn allow_path_for_file<'a>(
    rel_path: &Path,
    rel_path_str: &'a str,
    root_in_project: &Option<PathBuf>,
) -> AllowPath<'a> {
    if let Some(prefix) = root_in_project {
        let buf = prefix.join(rel_path);
        let s = buf.to_string_lossy().into_owned();
        AllowPath {
            _full: Some(buf),
            display: std::borrow::Cow::Owned(s),
        }
    } else {
        AllowPath {
            _full: None,
            display: std::borrow::Cow::Borrowed(rel_path_str),
        }
    }
}

/// Context needed to process matches for a single file.
struct FileContext<'a> {
    file: &'a Path,
    content: &'a str,
    source_registry: &'a SourceRegistry,
    source_ctx: SourceContext<'a>,
    allow_path_str: &'a str,
}

/// Process all query matches for a single file and append findings.
fn process_file_matches(
    ctx: &FileContext,
    tree: &tree_sitter::Tree,
    combined: &CombinedQuery,
    findings: &mut Vec<Finding>,
) {
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&combined.query, tree.root_node(), ctx.content.as_bytes());

    while let Some(m) = matches.next() {
        let Some((rule, match_idx)) = combined.pattern_to_rule.get(m.pattern_index) else {
            continue;
        };

        if rule.allow.iter().any(|p| p.matches(ctx.allow_path_str)) {
            continue;
        }

        if !rule.files.is_empty() {
            let filename = ctx
                .file
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            let matches_path = rule.files.iter().any(|p| p.matches(ctx.allow_path_str));
            let matches_name = rule.files.iter().any(|p| p.matches(filename.as_ref()));
            if !matches_path && !matches_name {
                continue;
            }
        }

        if !check_requires(rule, ctx.source_registry, &ctx.source_ctx) {
            continue;
        }

        if !evaluate_predicates(&combined.query, m, ctx.content.as_bytes()) {
            continue;
        }

        let Some(cap) = m.captures.iter().find(|c| c.index as usize == *match_idx) else {
            continue;
        };

        let start_line = cap.node.start_position().row + 1;
        if is_allowed_by_comment(ctx.content, start_line, &rule.id) {
            continue;
        }

        findings.push(build_finding(
            rule,
            cap.node,
            ctx.content,
            &combined.query,
            m,
            ctx.file,
        ));
    }
}

/// Run rules against files in a directory.
/// Optimized: combines all rules into single query per grammar for single-traversal matching.
#[allow(clippy::too_many_arguments)]
pub fn run_rules(
    rules: &[Rule],
    root: &Path,
    project_root: &Path,
    loader: &GrammarLoader,
    filter_rule: Option<&str>,
    filter_tag: Option<&str>,
    filter_ids: Option<&std::collections::HashSet<String>>,
    debug: &DebugFlags,
    files: Option<&[PathBuf]>,
    path_filter: &normalize_rules_config::PathFilter,
) -> Vec<Finding> {
    let start = std::time::Instant::now();
    let raw_abs_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let abs_root = if raw_abs_root.is_file() {
        raw_abs_root
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or(raw_abs_root)
    } else {
        raw_abs_root
    };
    let abs_project_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let root_in_project = abs_root
        .strip_prefix(&abs_project_root)
        .ok()
        .map(|p| p.to_path_buf());

    let mut findings = Vec::new();
    let source_registry = builtin_registry();

    let explicitly_requested = |r: &&Rule| {
        filter_rule.is_some_and(|f| r.id == f) || filter_ids.is_some_and(|ids| ids.contains(&r.id))
    };
    let active_rules: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.enabled || explicitly_requested(r))
        .filter(|r| filter_rule.is_none_or(|f| r.id == f))
        .filter(|r| filter_tag.is_none_or(|t| r.tags.iter().any(|tag| tag == t)))
        .filter(|r| filter_ids.is_none_or(|ids| ids.contains(&r.id)))
        .collect();

    if active_rules.is_empty() {
        return findings;
    }

    // Load the on-disk cache and invalidate if the rule set changed.
    let mut cache = SyntaxCache::load(&abs_project_root);
    let rules_hash = compute_rules_hash(&active_rules);
    if cache.rules_hash != rules_hash {
        cache.files.clear();
        cache.rules_hash = rules_hash;
    }

    let files = if let Some(explicit) = files {
        // Use the provided file list, filtering to supported languages.
        explicit
            .iter()
            .filter(|f| support_for_path(f).is_some())
            .cloned()
            .collect()
    } else {
        collect_source_files(root, path_filter)
    };
    let mut files_by_grammar: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for file in files {
        if let Some(lang) = support_for_path(&file) {
            let grammar_name = lang.grammar_name().to_string();
            files_by_grammar.entry(grammar_name).or_default().push(file);
        }
    }

    if debug.timing {
        eprintln!("[timing] file collection: {:?}", start.elapsed());
    }
    let compile_start = std::time::Instant::now();

    let (specific_rules, global_rules): (Vec<&&Rule>, Vec<&&Rule>) =
        active_rules.iter().partition(|r| !r.languages.is_empty());

    let mut combined_by_grammar: HashMap<String, CombinedQuery> = HashMap::new();
    for grammar_name in files_by_grammar.keys() {
        let Some(grammar) = loader.get(grammar_name).ok() else {
            continue;
        };
        if let Some(cq) =
            build_combined_query(grammar_name, &grammar, &specific_rules, &global_rules)
        {
            combined_by_grammar.insert(grammar_name.clone(), cq);
        }
    }

    if debug.timing {
        eprintln!(
            "[timing] query compilation: {:?} ({} grammars)",
            compile_start.elapsed(),
            combined_by_grammar.len()
        );
    }
    let process_start = std::time::Instant::now();

    for (grammar_name, files) in &files_by_grammar {
        let Some(combined) = combined_by_grammar.get(grammar_name) else {
            continue;
        };
        let Some(grammar) = loader.get(grammar_name).ok() else {
            continue;
        };
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&grammar).is_err() {
            continue;
        }

        for file in files {
            let file_key = file.to_string_lossy().into_owned();
            let mtime_nanos = file_mtime_nanos(file);

            // Cache hit: file is unchanged since the last run.
            if mtime_nanos > 0
                && let Some(entry) = cache.files.get(&file_key)
                && entry.mtime_nanos == mtime_nanos
            {
                findings.extend(entry.findings.iter().cloned().map(Finding::from));
                continue;
            }

            let rel_path = file.strip_prefix(root).unwrap_or(file);
            let rel_path_str = rel_path.to_string_lossy();

            let allow_path = allow_path_for_file(rel_path, &rel_path_str, &root_in_project);

            let Ok(content) = std::fs::read_to_string(file) else {
                continue;
            };
            let Some(tree) = parser.parse(&content, None) else {
                continue;
            };

            let file_ctx = FileContext {
                file,
                content: &content,
                source_registry: &source_registry,
                source_ctx: SourceContext {
                    file_path: file,
                    rel_path: &rel_path_str,
                    project_root: &abs_project_root,
                },
                allow_path_str: &allow_path.display,
            };

            let mut file_findings: Vec<Finding> = Vec::new();
            process_file_matches(&file_ctx, &tree, combined, &mut file_findings);

            // Update cache entry for this file.
            cache.files.insert(
                file_key,
                FileCacheEntry {
                    mtime_nanos,
                    findings: file_findings
                        .iter()
                        .cloned()
                        .map(CachedFinding::from)
                        .collect(),
                },
            );

            findings.extend(file_findings);
        }
    }

    if debug.timing {
        eprintln!(
            "[timing] file processing: {:?} ({} findings)",
            process_start.elapsed(),
            findings.len()
        );
        eprintln!("[timing] total: {:?}", start.elapsed());
    }

    cache.save(&abs_project_root);

    findings
}

/// Resolve a predicate argument to its text value.
fn resolve_arg_text<'a>(
    arg: &'a tree_sitter::QueryPredicateArg,
    match_: &tree_sitter::QueryMatch,
    source: &'a [u8],
) -> Option<&'a str> {
    match arg {
        tree_sitter::QueryPredicateArg::Capture(idx) => Some(
            match_
                .captures
                .iter()
                .find(|c| c.index == *idx)
                .and_then(|c| c.node.utf8_text(source).ok())
                .unwrap_or(""),
        ),
        tree_sitter::QueryPredicateArg::String(s) => Some(s.as_ref()),
    }
}

/// Resolve the first argument as a capture's text (not a string literal).
fn resolve_capture_text<'a>(
    arg: &'a tree_sitter::QueryPredicateArg,
    match_: &tree_sitter::QueryMatch,
    source: &'a [u8],
) -> Option<&'a str> {
    match arg {
        tree_sitter::QueryPredicateArg::Capture(idx) => Some(
            match_
                .captures
                .iter()
                .find(|c| c.index == *idx)
                .and_then(|c| c.node.utf8_text(source).ok())
                .unwrap_or(""),
        ),
        _ => None,
    }
}

/// Evaluate an eq?/not-eq? predicate. Returns None to skip, Some(false) to reject.
fn eval_eq(
    args: &[tree_sitter::QueryPredicateArg],
    match_: &tree_sitter::QueryMatch,
    source: &[u8],
    negated: bool,
) -> Option<bool> {
    if args.len() < 2 {
        return None;
    }
    let first = resolve_arg_text(&args[0], match_, source)?;
    let second = resolve_arg_text(&args[1], match_, source)?;
    let equal = first == second;
    Some(if negated { !equal } else { equal })
}

/// Evaluate a match?/not-match? predicate. Returns None to skip, Some(false) to reject.
fn eval_match(
    args: &[tree_sitter::QueryPredicateArg],
    match_: &tree_sitter::QueryMatch,
    source: &[u8],
    negated: bool,
) -> Option<bool> {
    if args.len() < 2 {
        return None;
    }
    let capture_text = resolve_capture_text(&args[0], match_, source)?;
    let pattern = match &args[1] {
        tree_sitter::QueryPredicateArg::String(s) => s.as_ref(),
        _ => return None,
    };
    let regex = regex::Regex::new(pattern).ok()?;
    let matched = regex.is_match(capture_text);
    Some(if negated { !matched } else { matched })
}

/// Evaluate an any-of? predicate. Returns None to skip, Some(false) to reject.
fn eval_any_of(
    args: &[tree_sitter::QueryPredicateArg],
    match_: &tree_sitter::QueryMatch,
    source: &[u8],
) -> Option<bool> {
    if args.len() < 2 {
        return None;
    }
    let capture_text = resolve_capture_text(&args[0], match_, source)?;
    let any_match = args[1..].iter().any(|arg| match arg {
        tree_sitter::QueryPredicateArg::String(s) => s.as_ref() == capture_text,
        _ => false,
    });
    Some(any_match)
}

/// Evaluate predicates for a match.
pub fn evaluate_predicates(
    query: &tree_sitter::Query,
    match_: &tree_sitter::QueryMatch,
    source: &[u8],
) -> bool {
    let predicates = query.general_predicates(match_.pattern_index);
    for predicate in predicates {
        let name = predicate.operator.as_ref();
        let args = &predicate.args;

        let result = match name {
            "eq?" => eval_eq(args, match_, source, false),
            "not-eq?" => eval_eq(args, match_, source, true),
            "match?" => eval_match(args, match_, source, false),
            "not-match?" => eval_match(args, match_, source, true),
            "any-of?" => eval_any_of(args, match_, source),
            _ => None,
        };

        // None means skip (bad args), Some(false) means predicate failed
        if result == Some(false) {
            return false;
        }
    }
    true
}

#[cfg(feature = "fix")]
/// Expand a fix template by substituting capture names with their values.
/// Uses `$capture_name` syntax. `$match` is the full matched text.
pub fn expand_fix_template(template: &str, captures: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (name, value) in captures {
        let placeholder = format!("${}", name);
        result = result.replace(&placeholder, value);
    }
    result
}

#[cfg(feature = "fix")]
/// Apply one pass of fixes to findings, returning the number of files modified.
///
/// Fixes are applied in descending byte-offset order within each file so that
/// earlier offsets remain valid as later regions are replaced.
///
/// When findings overlap (e.g. a nested triple `if let` produces both an inner
/// and an outer violation), the innermost finding (highest `start_byte`) is
/// applied first and the outer one is skipped for this pass.  The caller
/// should re-run the rules and call `apply_fixes` again until no files are
/// modified; each pass peels one layer of nesting.
pub fn apply_fixes(findings: &[Finding]) -> std::io::Result<usize> {
    // Group findings by file
    let mut by_file: HashMap<&PathBuf, Vec<&Finding>> = HashMap::new();
    for finding in findings {
        if finding.fix.is_some() {
            by_file.entry(&finding.file).or_default().push(finding);
        }
    }

    let mut files_modified = 0;

    for (file, mut file_findings) in by_file {
        // Descending start_byte: innermost (highest offset) findings are
        // processed first, so their replacements don't shift the offsets of
        // earlier findings in the same file.
        file_findings.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

        let mut content = std::fs::read_to_string(file)?;
        // Track byte ranges that have already been replaced in this pass.
        // Any finding whose range overlaps an applied range is skipped — it
        // is an outer wrapper of an already-fixed inner finding, and its
        // captures are stale.  The next pass will pick it up with fresh
        // byte offsets.
        let mut applied: Vec<(usize, usize)> = Vec::new();
        let mut file_changed = false;

        for finding in file_findings {
            let overlaps = applied
                .iter()
                .any(|&(s, e)| finding.start_byte < e && finding.end_byte > s);
            if overlaps {
                continue;
            }

            // fix.is_some() is guaranteed: by_file only includes findings where fix.is_some()
            let Some(fix_template) = finding.fix.as_ref() else {
                continue;
            };
            let replacement = expand_fix_template(fix_template, &finding.captures);

            let before = &content[..finding.start_byte];
            let after = &content[finding.end_byte..];
            content = format!("{}{}{}", before, replacement, after);

            applied.push((finding.start_byte, finding.end_byte));
            file_changed = true;
        }

        if file_changed {
            std::fs::write(file, &content)?;
            files_modified += 1;
        }
    }

    Ok(files_modified)
}

/// Collect source files from a directory, optionally filtered by [`PathFilter`].
fn collect_source_files(root: &Path, filter: &normalize_rules_config::PathFilter) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && support_for_path(path).is_some() {
            if !filter.is_empty() {
                let rel = path.strip_prefix(root).unwrap_or(path);
                if !filter.matches_path(rel) {
                    continue;
                }
            }
            files.push(path.to_path_buf());
        }
    }

    files
}

/// Split a tree-sitter query string into individual top-level patterns.
/// Each pattern starts with `(` at the beginning of a line (possibly after
/// whitespace/comments) and ends when the matching `)` is found.
fn split_query_patterns(query_str: &str) -> Vec<&str> {
    let mut patterns = Vec::new();
    let mut depth = 0i32;
    let mut pattern_start: Option<usize> = None;
    let bytes = query_str.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b';' => {
                // Skip to end of line (comment)
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'(' => {
                if pattern_start.is_none() {
                    pattern_start = Some(i);
                }
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0
                    && let Some(start) = pattern_start
                {
                    patterns.push(&query_str[start..i]);
                    pattern_start = None;
                }
            }
            b'"' => {
                // Skip string literal
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
                i += 1; // skip closing quote
            }
            _ => {
                i += 1;
            }
        }
    }
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;
    use normalize_languages::GrammarLoader;
    use normalize_languages::parsers::grammar_loader;
    use std::sync::Arc;
    use streaming_iterator::StreamingIterator;

    fn loader() -> Arc<GrammarLoader> {
        grammar_loader()
    }

    /// Test that combined queries correctly scope predicates per-pattern.
    #[test]
    fn test_combined_query_predicate_scoping() {
        let loader = loader();
        let grammar = loader.get("rust").expect("rust grammar");

        // Two patterns with same capture name but different predicate values
        let combined_query = r#"
; Pattern 0: matches unwrap
((call_expression
  function: (field_expression field: (field_identifier) @_method)
  (#eq? @_method "unwrap")) @match)

; Pattern 1: matches expect
((call_expression
  function: (field_expression field: (field_identifier) @_method)
  (#eq? @_method "expect")) @match)
"#;

        let query = tree_sitter::Query::new(&grammar, combined_query)
            .expect("combined query should compile");

        assert_eq!(query.pattern_count(), 2, "should have 2 patterns");

        let test_code = r#"
fn main() {
    let x = Some(5);
    x.unwrap();      // line 4 - should match pattern 0
    x.expect("msg"); // line 5 - should match pattern 1
    x.map(|v| v);    // line 6 - should NOT match
}
"#;

        let mut parser = tree_sitter::Parser::new();
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        parser.set_language(&grammar).unwrap();
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let tree = parser.parse(test_code, None).unwrap();

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), test_code.as_bytes());

        let mut results: Vec<(usize, String)> = Vec::new();
        while let Some(m) = matches.next() {
            // Check predicates - this is what we're testing
            if !evaluate_predicates(&query, m, test_code.as_bytes()) {
                continue;
            }

            let match_capture = m
                .captures
                .iter()
                .find(|c| query.capture_names()[c.index as usize] == "match");

            if let Some(cap) = match_capture {
                // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
                let text = cap.node.utf8_text(test_code.as_bytes()).unwrap();
                results.push((m.pattern_index, text.to_string()));
            }
        }

        // Should have exactly 2 matches
        assert_eq!(results.len(), 2, "should have 2 matches, got {:?}", results);

        // Pattern 0 should match unwrap
        assert!(
            results
                .iter()
                .any(|(idx, text)| *idx == 0 && text.contains("unwrap")),
            "pattern 0 should match unwrap, got {:?}",
            results
        );

        // Pattern 1 should match expect
        assert!(
            results
                .iter()
                .any(|(idx, text)| *idx == 1 && text.contains("expect")),
            "pattern 1 should match expect, got {:?}",
            results
        );
    }

    /// Test that multiple rules can be combined into single query.
    #[test]
    fn test_combined_rules_single_traversal() {
        let loader = loader();
        let grammar = loader.get("rust").expect("rust grammar");

        // Simulate combining multiple rule queries
        let rules_queries = [
            (
                "unwrap-rule",
                r#"((call_expression function: (field_expression field: (field_identifier) @_m) (#eq? @_m "unwrap")) @match)"#,
            ),
            (
                "dbg-rule",
                r#"((macro_invocation macro: (identifier) @_name (#eq? @_name "dbg")) @match)"#,
            ),
        ];

        // Combine into single query
        let combined = rules_queries
            .iter()
            .map(|(_, q)| *q)
            .collect::<Vec<_>>()
            .join("\n\n");

        let query =
            tree_sitter::Query::new(&grammar, &combined).expect("combined query should compile");

        let test_code = r#"
fn main() {
    let x = Some(5);
    dbg!(x);        // should match pattern 1 (dbg-rule)
    x.unwrap();     // should match pattern 0 (unwrap-rule)
}
"#;

        let mut parser = tree_sitter::Parser::new();
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        parser.set_language(&grammar).unwrap();
        // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
        let tree = parser.parse(test_code, None).unwrap();

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), test_code.as_bytes());

        let mut pattern_indices: Vec<usize> = Vec::new();
        while let Some(m) = matches.next() {
            if evaluate_predicates(&query, m, test_code.as_bytes()) {
                pattern_indices.push(m.pattern_index);
            }
        }

        // Should match both patterns
        assert!(
            pattern_indices.contains(&0),
            "should match pattern 0 (unwrap)"
        );
        assert!(pattern_indices.contains(&1), "should match pattern 1 (dbg)");
    }

    #[test]
    fn test_split_query_patterns() {
        let query = r#"
; Pattern 1: comment
((comment) @match (#match? @match "TODO"))
; Pattern 2: line_comment
((line_comment) @match (#match? @match "TODO"))
"#;
        let patterns = split_query_patterns(query);
        assert_eq!(patterns.len(), 2);
        assert!(patterns[0].contains("comment"));
        assert!(patterns[1].contains("line_comment"));
    }

    #[test]
    fn test_cross_grammar_pattern_fallback() {
        // Rust grammar doesn't have `comment` node type but has `line_comment`.
        // A multi-pattern query with both should compile with only valid patterns.
        let loader = loader();
        let grammar = loader.get("rust").expect("rust grammar");

        let query_str = r#"((comment) @match (#match? @match "TODO"))
((line_comment) @match (#match? @match "TODO"))"#;

        // Full query should fail (Rust has no `comment` node type)
        assert!(tree_sitter::Query::new(&grammar, query_str).is_err());

        // But splitting and filtering should succeed
        let patterns = split_query_patterns(query_str);
        let valid: Vec<&str> = patterns
            .into_iter()
            .filter(|p| tree_sitter::Query::new(&grammar, p).is_ok())
            .collect();
        assert_eq!(valid.len(), 1, "only line_comment should compile for Rust");
        assert!(valid[0].contains("line_comment"));
    }
}

#[cfg(test)]
mod glob_tests {
    use glob::Pattern;
    // normalize-syntax-allow: rust/unwrap-in-impl - test code, panic is appropriate
    #[test]
    fn test_glob_allow_patterns() {
        let cases = [
            (
                "crates/normalize/src/rg/**",
                "crates/normalize/src/rg/flags/defs.rs",
                true,
            ),
            (
                "crates/normalize/src/rg/**",
                "crates/normalize/src/rg/mod.rs",
                true,
            ),
            ("**/tests/**", "crates/normalize/tests/foo.rs", true),
            (
                "**/tests/fixtures/**",
                "crates/normalize-syntax-rules/tests/fixtures/rust/foo.rs",
                true,
            ),
            (
                "crates/normalize-facts-rules-interpret/src/tests.rs",
                "crates/normalize-facts-rules-interpret/src/tests.rs",
                true,
            ),
            (
                "crates/normalize-manifest/src/*.rs",
                "crates/normalize-manifest/src/nuget.rs",
                true,
            ),
        ];
        for (p, path, expected) in cases {
            // normalize-syntax-allow: rust/unwrap-in-impl - test code, literal constant patterns
            let pat = Pattern::new(p).unwrap();
            assert_eq!(pat.matches(path), expected, "Pattern: {p}, Path: {path}");
        }
    }
}
