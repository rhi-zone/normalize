//! Merge trait for configuration layering.
//!
//! Used to merge global config with project config, where "other" (project) wins.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

/// Trait for merging configuration values.
///
/// Convention: `other` takes precedence over `self`.
/// For Option types, `other` wins if Some, otherwise falls back to `self`.
pub trait Merge {
    fn merge(self, other: Self) -> Self;
}

// Re-export the derive macro
pub use moss_derive::Merge;

// === Primitives (other always wins) ===

impl Merge for bool {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for i8 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for i16 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for i32 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for i64 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for i128 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for isize {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for u8 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for u16 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for u32 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for u64 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for u128 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for usize {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for f32 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for f64 {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for char {
    fn merge(self, other: Self) -> Self {
        other
    }
}

// === Strings ===

impl Merge for String {
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl Merge for PathBuf {
    fn merge(self, other: Self) -> Self {
        other
    }
}

// === Option: merge inner values if both Some ===

impl<T: Merge> Merge for Option<T> {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Some(a), Some(b)) => Some(a.merge(b)),
            (None, b) => b,
            (a, None) => a,
        }
    }
}

// === Collections: extend/merge ===

impl<T> Merge for Vec<T> {
    /// Vectors: other replaces self entirely (not appended)
    fn merge(self, other: Self) -> Self {
        other
    }
}

impl<K: Eq + std::hash::Hash, V> Merge for HashMap<K, V> {
    /// HashMaps: other's keys override self's
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

impl<K: Ord, V> Merge for BTreeMap<K, V> {
    /// BTreeMaps: other's keys override self's
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

impl<T: Eq + std::hash::Hash> Merge for HashSet<T> {
    /// HashSets: union
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

impl<T: Ord> Merge for BTreeSet<T> {
    /// BTreeSets: union
    fn merge(mut self, other: Self) -> Self {
        self.extend(other);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_merge() {
        assert!(false.merge(true));
        assert!(!true.merge(false));
    }

    #[test]
    fn test_option_merge() {
        assert_eq!(None::<i32>.merge(Some(1)), Some(1));
        assert_eq!(Some(1).merge(None), Some(1));
        assert_eq!(Some(1).merge(Some(2)), Some(2)); // inner merge: other wins for primitives
    }

    #[test]
    fn test_option_hashmap_merge() {
        // Option<HashMap> should merge inner hashmaps when both Some
        let a: Option<HashMap<&str, i32>> = Some([("x", 1), ("y", 2)].into_iter().collect());
        let b: Option<HashMap<&str, i32>> = Some([("y", 3), ("z", 4)].into_iter().collect());

        let merged = a.merge(b).unwrap();
        assert_eq!(merged.get("x"), Some(&1)); // from a
        assert_eq!(merged.get("y"), Some(&3)); // b wins
        assert_eq!(merged.get("z"), Some(&4)); // from b
    }

    #[test]
    fn test_hashmap_merge() {
        let mut a = HashMap::new();
        a.insert("a", 1);
        a.insert("b", 2);

        let mut b = HashMap::new();
        b.insert("b", 3);
        b.insert("c", 4);

        let merged = a.merge(b);
        assert_eq!(merged.get("a"), Some(&1));
        assert_eq!(merged.get("b"), Some(&3)); // b wins
        assert_eq!(merged.get("c"), Some(&4));
    }
}
