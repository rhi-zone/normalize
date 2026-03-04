//! Code similarity algorithms: MinHash + LSH, normalized AST hashing, and structural tokenization.
//!
//! Shared between `normalize analyze duplicate-functions`, `normalize analyze similar-functions`,
//! `normalize analyze similar-blocks`, and `normalize analyze patterns`.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

// ── MinHash + LSH constants ───────────────────────────────────────────────────

pub const MINHASH_N: usize = 128;
pub const SHINGLE_K: usize = 3;
/// 32 bands × 4 rows → good recall at ≥0.7 similarity
pub const LSH_BANDS: usize = 32;
pub const LSH_ROWS: usize = 4; // MINHASH_N / LSH_BANDS

// ── MinHash + LSH ─────────────────────────────────────────────────────────────

/// Simple universal hash for MinHash: mixes x with a per-function seed.
#[inline]
pub fn minhash_hash(x: u64, seed: u64) -> u64 {
    let a = 6364136223846793005u64.wrapping_add(seed.wrapping_mul(2654435761));
    let b = 1442695040888963407u64.wrapping_add(seed.wrapping_mul(1013904223));
    a.wrapping_mul(x).wrapping_add(b)
}

/// Compute a MinHash signature over k-shingles of the token sequence.
pub fn compute_minhash(tokens: &[u64]) -> [u64; MINHASH_N] {
    let mut sig = [u64::MAX; MINHASH_N];
    if tokens.len() < SHINGLE_K {
        return sig;
    }
    for window in tokens.windows(SHINGLE_K) {
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        window.hash(&mut h);
        let shingle_hash = h.finish();

        for (i, slot) in sig.iter_mut().enumerate() {
            let v = minhash_hash(shingle_hash, i as u64);
            if v < *slot {
                *slot = v;
            }
        }
    }
    sig
}

/// Estimate Jaccard similarity from two MinHash signatures.
pub fn jaccard_estimate(a: &[u64; MINHASH_N], b: &[u64; MINHASH_N]) -> f64 {
    let matches = a.iter().zip(b.iter()).filter(|(x, y)| x == y).count();
    matches as f64 / MINHASH_N as f64
}

/// Hash one LSH band of a signature.
pub fn lsh_band_hash(sig: &[u64; MINHASH_N], band: usize) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let start = band * LSH_ROWS;
    let mut h = DefaultHasher::new();
    band.hash(&mut h);
    for v in &sig[start..start + LSH_ROWS] {
        v.hash(&mut h);
    }
    h.finish()
}

// ── Normalized AST hashing ────────────────────────────────────────────────────

/// Compute a normalized AST hash for duplicate function detection.
/// Hashes the tree structure; identifiers and/or literals can be elided
/// to detect semantic duplicates regardless of naming.
pub fn compute_function_hash(
    node: &tree_sitter::Node,
    content: &[u8],
    elide_identifiers: bool,
    elide_literals: bool,
) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    hash_node_recursive(
        node,
        content,
        &mut hasher,
        elide_identifiers,
        elide_literals,
    );
    hasher.finish()
}

/// Recursively hash a node and its children.
pub fn hash_node_recursive(
    node: &tree_sitter::Node,
    content: &[u8],
    hasher: &mut impl Hasher,
    elide_identifiers: bool,
    elide_literals: bool,
) {
    let kind = node.kind();

    // Hash the node kind (structure)
    kind.hash(hasher);

    // For leaf nodes, decide whether to hash content
    if node.child_count() == 0 {
        let should_hash = if is_identifier_kind(kind) {
            !elide_identifiers
        } else if is_literal_kind(kind) {
            !elide_literals
        } else {
            // Operators, keywords — their kind is sufficient
            false
        };

        if should_hash {
            let text = &content[node.start_byte()..node.end_byte()];
            text.hash(hasher);
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        hash_node_recursive(&child, content, hasher, elide_identifiers, elide_literals);
    }
}

// ── Identifier / literal classification ──────────────────────────────────────

/// Check if a node kind represents an identifier.
pub fn is_identifier_kind(kind: &str) -> bool {
    kind == "identifier"
        || kind == "field_identifier"
        || kind == "type_identifier"
        || kind == "property_identifier"
        || kind.ends_with("_identifier")
}

/// Check if a node kind represents a literal value.
pub fn is_literal_kind(kind: &str) -> bool {
    kind.contains("string")
        || kind.contains("integer")
        || kind.contains("float")
        || kind.contains("number")
        || kind.contains("boolean")
        || kind == "true"
        || kind == "false"
        || kind == "nil"
        || kind == "null"
        || kind == "none"
}

// ── AST token serialization ───────────────────────────────────────────────────

/// Returns true for node kinds that represent a block/body.
/// Used by skeleton mode to replace the entire subtree with a `<body>` placeholder token.
pub fn is_body_kind(kind: &str) -> bool {
    matches!(
        kind,
        "block"                    // Rust, Go, many others
        | "body"                   // Python, Kotlin
        | "statement_block"        // JavaScript, TypeScript
        | "compound_statement"     // C, C++, Bash
        | "declaration_list"       // C/C++ struct/union body
        | "field_declaration_list" // Rust struct body
        | "enum_body"              // Java, Kotlin
        | "class_body"             // Java, Kotlin, TypeScript
        | "interface_body"         // Java
        | "object_body"            // Kotlin
        | "do_block"               // Ruby
        | "begin_block"            // Ruby
        | "block_body" // generic
    ) || kind.ends_with("_body")
        || kind.ends_with("_block")
        || kind.ends_with("_list") && kind.contains("statement")
}

/// Hash token for skeleton body placeholder — a fixed sentinel value.
pub const BODY_PLACEHOLDER: u64 = 0xb0d7_b0d7_b0d7_b0d7;

/// Serialize an AST subtree to a flat token sequence for shingling.
/// In skeleton mode, body/block subtrees are replaced with a fixed placeholder token.
pub fn serialize_subtree_tokens(
    node: &tree_sitter::Node,
    content: &[u8],
    elide_identifiers: bool,
    elide_literals: bool,
    skeleton: bool,
    out: &mut Vec<u64>,
) {
    use std::collections::hash_map::DefaultHasher;
    let kind = node.kind();

    // In skeleton mode, replace body/block subtrees with a fixed placeholder.
    if skeleton && node.child_count() > 0 && is_body_kind(kind) {
        out.push(BODY_PLACEHOLDER);
        return;
    }

    let mut h = DefaultHasher::new();
    kind.hash(&mut h);

    if node.child_count() == 0 {
        let should_include = if is_identifier_kind(kind) {
            !elide_identifiers
        } else if is_literal_kind(kind) {
            !elide_literals
        } else {
            false
        };
        if should_include {
            let text = &content[node.start_byte()..node.end_byte()];
            text.hash(&mut h);
        }
    }
    out.push(h.finish());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        serialize_subtree_tokens(
            &child,
            content,
            elide_identifiers,
            elide_literals,
            skeleton,
            out,
        );
    }
}

// ── Structural token extraction (for pattern analysis) ────────────────────────

/// Walk an AST node and emit hashes only for structural (control-flow, call,
/// assignment) node kinds. Everything else is ignored.
pub fn serialize_structural_tokens(
    node: &tree_sitter::Node,
    structural_kinds: &HashSet<&str>,
    out: &mut Vec<u64>,
) {
    let kind = node.kind();

    let is_structural =
        structural_kinds.contains(kind) || kind.contains("call") || kind.contains("assignment");

    if is_structural {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        kind.hash(&mut h);
        out.push(h.finish());
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            serialize_structural_tokens(&cursor.node(), structural_kinds, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Collect structural node kind counts from an AST subtree.
pub fn collect_structural_kinds(
    node: &tree_sitter::Node,
    structural_kinds: &HashSet<&str>,
    counts: &mut HashMap<String, usize>,
) {
    let kind = node.kind();
    if structural_kinds.contains(kind) || kind.contains("call") || kind.contains("assignment") {
        *counts.entry(kind.to_string()).or_default() += 1;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_structural_kinds(&cursor.node(), structural_kinds, counts);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

// ── Pattern classification ────────────────────────────────────────────────────

/// Categorize a node kind into a readable label for pattern naming.
pub fn categorize_kind(kind: &str) -> &str {
    if kind.contains("if") || kind.contains("match") || kind.contains("switch") {
        "branch"
    } else if kind.contains("for") || kind.contains("while") || kind.contains("loop") {
        "loop"
    } else if kind.contains("try") || kind.contains("catch") || kind.contains("rescue") {
        "error-handling"
    } else if kind.contains("return") || kind.contains("break") || kind.contains("continue") {
        "exit"
    } else if kind.contains("call") {
        "call"
    } else if kind.contains("assignment") {
        "transform"
    } else {
        "control"
    }
}

/// Generate a human-readable label from structural element counts.
pub fn generate_pattern_label(elements: &[(String, usize)]) -> String {
    if elements.is_empty() {
        return "unknown".to_string();
    }

    // Categorize and aggregate
    let mut categories: HashMap<&str, usize> = HashMap::new();
    for (kind, count) in elements {
        let cat = categorize_kind(kind);
        *categories.entry(cat).or_default() += count;
    }

    // Sort by count descending, pick top 2
    let mut sorted: Vec<(&str, usize)> = categories.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let parts: Vec<&str> = sorted.iter().take(2).map(|(cat, _)| *cat).collect();

    match parts.len() {
        0 => "unknown".to_string(),
        1 => {
            let total: usize = elements.iter().map(|(_, c)| c).sum();
            if total > 6 {
                format!("{}-heavy", parts[0])
            } else {
                parts[0].to_string()
            }
        }
        _ => format!("{}-{}", parts[0], parts[1]),
    }
}

// ── Union-Find ────────────────────────────────────────────────────────────────

/// Union-Find / Disjoint Set Union data structure for clustering.
pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    pub fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    pub fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return;
        }
        match self.rank[rx].cmp(&self.rank[ry]) {
            std::cmp::Ordering::Less => self.parent[rx] = ry,
            std::cmp::Ordering::Greater => self.parent[ry] = rx,
            std::cmp::Ordering::Equal => {
                self.parent[ry] = rx;
                self.rank[rx] += 1;
            }
        }
    }
}

// ── Tree-sitter helpers ───────────────────────────────────────────────────────

/// Flatten nested symbols into a flat list.
pub fn flatten_symbols(sym: &normalize_languages::Symbol) -> Vec<&normalize_languages::Symbol> {
    let mut result = vec![sym];
    for child in &sym.children {
        result.extend(flatten_symbols(child));
    }
    result
}

/// Find the function/method node at a given (1-based) line in the tree.
pub fn find_function_node(
    tree: &tree_sitter::Tree,
    target_line: usize,
) -> Option<tree_sitter::Node<'_>> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    find_node_at_line_recursive(&mut cursor, target_line)
}

fn find_node_at_line_recursive<'a>(
    cursor: &mut tree_sitter::TreeCursor<'a>,
    target_line: usize,
) -> Option<tree_sitter::Node<'a>> {
    loop {
        let node = cursor.node();
        let start = node.start_position().row + 1;

        if start == target_line {
            let kind = node.kind();
            if kind.contains("function")
                || kind.contains("method")
                || kind == "function_definition"
                || kind == "method_definition"
                || kind == "function_item"
                || kind == "function_declaration"
                || kind == "arrow_function"
                || kind == "generator_function"
            {
                return Some(node);
            }
        }

        if cursor.goto_first_child() {
            if let Some(found) = find_node_at_line_recursive(cursor, target_line) {
                return Some(found);
            }
            cursor.goto_parent();
        }

        if !cursor.goto_next_sibling() {
            break;
        }
    }
    None
}
