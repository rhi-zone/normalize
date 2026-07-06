//! Output formatting utilities (re-exported from normalize-output).

// `tier_color` and `pretty_ranked_table` moved to `normalize-output` (alongside
// `OutputFormatter` and the `nu_ansi_term` dependency) so both this crate and
// feature crates like `normalize-git-history` can render pretty rank tables.
// Re-exported here via `pub use normalize_output::*` so existing
// `crate::output::{tier_color, pretty_ranked_table}` call sites keep resolving.
pub use normalize_output::*;
