# normalize-code-similarity/src

Single-file source for the `normalize-code-similarity` crate.

`lib.rs` contains all similarity algorithms: MinHash + LSH primitives, normalized AST hashing (`compute_function_hash`, `hash_node_recursive`), AST token serialization for shingling (`serialize_subtree_tokens`, `serialize_structural_tokens`), structural pattern helpers (`is_body_kind`, `categorize_kind`, `generate_pattern_label`), `UnionFind` for clustering, and `find_function_node` for locating functions in a tree-sitter tree by line number.
