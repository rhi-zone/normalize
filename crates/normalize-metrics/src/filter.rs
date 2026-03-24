//! Path-prefix filtering for metric address pairs.

/// A single metric measurement returned by [`filter_by_prefix`].
pub struct MetricPoint<'a> {
    /// The address (path) of this metric item.
    pub address: &'a str,
    /// The measured value.
    pub value: f64,
}

/// Filter `(key, value)` pairs to those whose key matches the given path prefix.
///
/// Matching rules:
/// - Exact match: `addr == prefix` (after stripping trailing slash from prefix)
/// - Child match: `addr` starts with `prefix/`
///
/// Trailing slashes on `prefix` are stripped before comparison, so `"src/"` and `"src"` match
/// identically.
pub fn filter_by_prefix<'a>(
    items: &'a [(String, f64)],
    prefix: &str,
) -> impl Iterator<Item = MetricPoint<'a>> {
    // Normalise: strip any trailing slash to get the canonical prefix.
    let canonical = prefix.trim_end_matches('/').to_string();

    items
        .iter()
        .filter(move |(addr, _)| {
            // Exact match, or child path separated by '/'.
            addr.as_str() == canonical.as_str() || addr.starts_with(&format!("{canonical}/"))
        })
        .map(|(addr, value)| MetricPoint {
            address: addr.as_str(),
            value: *value,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_exact() {
        let items = vec![
            ("src/main.rs".to_string(), 1.0),
            ("src/lib.rs".to_string(), 2.0),
        ];
        let result: Vec<_> = filter_by_prefix(&items, "src/main.rs").collect();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].address, "src/main.rs");
    }

    #[test]
    fn test_filter_prefix() {
        let items = vec![
            ("src/main.rs".to_string(), 1.0),
            ("src/lib.rs".to_string(), 2.0),
            ("tests/foo.rs".to_string(), 3.0),
        ];
        let result: Vec<_> = filter_by_prefix(&items, "src").collect();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_no_partial_match() {
        // "src" should not match "srcother"
        let items = vec![("srcother/main.rs".to_string(), 1.0)];
        let result: Vec<_> = filter_by_prefix(&items, "src").collect();
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_trailing_slash() {
        // "src/" should match addresses starting with "src/"
        let items = vec![
            ("src/main.rs".to_string(), 1.0),
            ("src/lib.rs".to_string(), 2.0),
            ("tests/foo.rs".to_string(), 3.0),
        ];
        let result: Vec<_> = filter_by_prefix(&items, "src/").collect();
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.address.starts_with("src/")));
    }
}
