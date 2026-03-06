# normalize-code-similarity

Code similarity algorithms shared between `normalize analyze duplicates` and `normalize analyze fragments`.

Implements MinHash LSH for approximate Jaccard similarity, normalized AST hashing (with optional identifier/literal elision), structural token serialization, skeleton-mode body replacement, and a union-find data structure for clustering. Key functions: `compute_minhash`, `jaccard_estimate`, `lsh_band_hash`, `compute_function_hash`, `serialize_subtree_tokens`, `serialize_structural_tokens`, `find_function_node`. Constants: `MINHASH_N=128`, `LSH_BANDS=32`, `LSH_ROWS=4`.
