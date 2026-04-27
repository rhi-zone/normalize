//! Predicate evaluation for tree-sitter queries.
//!
//! Tree-sitter compiles `#match?`, `#eq?`, etc. into `QueryPredicate` structs but
//! does **not** evaluate them at match time — the caller is responsible for
//! filtering matches that fail their predicates.
//!
//! [`satisfies_predicates`] evaluates the standard tree-sitter predicates so that
//! query authors can use them in `.scm` files and have them honoured at runtime.

use tree_sitter::{Query, QueryMatch, QueryPredicateArg};

/// Return `true` if all predicates on `m`'s pattern are satisfied, `false` otherwise.
///
/// Supported predicates:
/// - `#match?` — captured text must match the regex
/// - `#not-match?` — captured text must not match the regex
/// - `#eq?` — two captures/strings must be equal
/// - `#not-eq?` — two captures/strings must not be equal
///
/// Unknown predicates pass (return `true`) so future predicates don't break existing
/// queries.
pub fn satisfies_predicates(query: &Query, m: &QueryMatch, source: &[u8]) -> bool {
    for predicate in query.general_predicates(m.pattern_index) {
        let op = predicate.operator.as_ref();
        match op {
            "match?" | "not-match?" => {
                let args = &predicate.args;
                if args.len() != 2 {
                    continue;
                }
                let capture_index = match &args[0] {
                    QueryPredicateArg::Capture(idx) => *idx,
                    _ => continue,
                };
                let pattern = match &args[1] {
                    QueryPredicateArg::String(s) => s.as_ref(),
                    _ => continue,
                };
                let text = capture_text(m, capture_index, source);
                let matches = regex_matches(pattern, text);
                let want_match = op == "match?";
                if matches != want_match {
                    return false;
                }
            }
            "eq?" | "not-eq?" => {
                let args = &predicate.args;
                if args.len() != 2 {
                    continue;
                }
                let lhs = resolve_arg(&args[0], m, source);
                let rhs = resolve_arg(&args[1], m, source);
                let equal = lhs == rhs;
                let want_eq = op == "eq?";
                if equal != want_eq {
                    return false;
                }
            }
            // Unknown predicates pass so future predicates don't break existing queries.
            _ => {}
        }
    }
    true
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn capture_text<'a>(m: &QueryMatch, capture_index: u32, source: &'a [u8]) -> &'a str {
    m.captures
        .iter()
        .find(|c| c.index == capture_index)
        .and_then(|c| c.node.utf8_text(source).ok())
        .unwrap_or("")
}

fn resolve_arg<'a>(arg: &QueryPredicateArg, m: &'a QueryMatch, source: &'a [u8]) -> &'a str {
    match arg {
        QueryPredicateArg::Capture(idx) => capture_text(m, *idx, source),
        QueryPredicateArg::String(s) => {
            // SAFETY: we extend the lifetime here — the string is borrowed from the
            // predicate which lives as long as the Query, which outlives this call.
            // Callers hold the Query for the duration of the loop so this is safe.
            unsafe { std::mem::transmute::<&str, &'a str>(s.as_ref()) }
        }
    }
}

/// Test whether `text` matches `pattern` as a regex.
///
/// Errors (invalid regex) are treated as non-matching so a bad predicate doesn't panic.
fn regex_matches(pattern: &str, text: &str) -> bool {
    // Compile the regex on each call. In practice predicates are called on every
    // match so a caching approach would be better for hot paths, but this is correct
    // and avoids adding a HashMap dependency to this helper.
    match regex::Regex::new(pattern) {
        Ok(re) => re.is_match(text),
        Err(_) => false,
    }
}
