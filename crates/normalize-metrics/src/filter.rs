//! Path-prefix filtering for metric address pairs.

/// Filter `(key, value)` pairs to those whose key matches the given path prefix.
///
/// Matching rules:
/// - Exact match: `addr == prefix`
/// - Child match: `addr` starts with `prefix/`
/// - Trailing-slash prefix: `addr` starts with `prefix` when prefix ends with `/`
pub fn filter_by_prefix<'a>(
    items: &'a [(String, f64)],
    prefix: &str,
) -> impl Iterator<Item = &'a (String, f64)> {
    let prefix = prefix.trim_end_matches('/').to_string();
    items.iter().filter(move |(addr, _)| {
        addr == &prefix
            || addr.starts_with(&format!("{prefix}/"))
            || (prefix.ends_with('/') && addr.starts_with(&prefix))
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
        assert_eq!(result[0].0, "src/main.rs");
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
}
