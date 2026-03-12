//! Ranked list helpers: scoring, stats, and the shared rank pipeline.
//!
//! ## Table formatting
//!
//! The [`RankEntry`] trait + [`format_ranked_table`] function provide shared
//! tabular rendering for all rank-pattern commands. Implement `RankEntry` on
//! your entry struct, then call `format_ranked_table()` in your
//! `OutputFormatter::format_text()`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;

use crate::Entity;

// ── Column / alignment types ───────────────────────────────────────────

/// Column alignment in a ranked table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// Column definition for ranked table rendering.
#[derive(Debug, Clone)]
pub struct Column {
    pub name: &'static str,
    pub align: Align,
}

impl Column {
    pub const fn left(name: &'static str) -> Self {
        Self {
            name,
            align: Align::Left,
        }
    }

    pub const fn right(name: &'static str) -> Self {
        Self {
            name,
            align: Align::Right,
        }
    }
}

// ── RankEntry trait ────────────────────────────────────────────────────

/// Trait for entries that can be rendered in a ranked table.
///
/// Implement this on your entry struct to use [`format_ranked_table`].
pub trait RankEntry {
    /// Column definitions for the table header.
    fn columns() -> Vec<Column>;

    /// Format this entry's values as strings, one per column, in column order.
    fn values(&self) -> Vec<String>;
}

// ── Table formatter ────────────────────────────────────────────────────

/// Render a ranked list as a text table.
///
/// Produces: title line, blank line, column headers, separator, rows.
/// If `entries` is empty, shows `empty_message` (or a default).
pub fn format_ranked_table<E: RankEntry>(
    title: &str,
    entries: &[E],
    empty_message: Option<&str>,
) -> String {
    let mut out = Vec::new();

    out.push(title.to_string());
    out.push(String::new());

    if entries.is_empty() {
        out.push(empty_message.unwrap_or("No entries.").to_string());
        return out.join("\n");
    }

    let cols = E::columns();

    // Pre-compute all values so we can measure widths
    let all_values: Vec<Vec<String>> = entries.iter().map(|e| e.values()).collect();

    // Compute column widths: max(header, all data)
    let widths: Vec<usize> = cols
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let header_w = col.name.len();
            let data_w = all_values
                .iter()
                .map(|row| row.get(i).map_or(0, |v| v.len()))
                .max()
                .unwrap_or(0);
            header_w.max(data_w)
        })
        .collect();

    // Header row
    let header: String = cols
        .iter()
        .zip(&widths)
        .map(|(col, &w)| match col.align {
            Align::Left => format!("{:<width$}", col.name, width = w),
            Align::Right => format!("{:>width$}", col.name, width = w),
        })
        .collect::<Vec<_>>()
        .join("  ");
    out.push(header);

    // Separator
    let sep: String = widths
        .iter()
        .map(|&w| "-".repeat(w))
        .collect::<Vec<_>>()
        .join("--");
    out.push(sep);

    // Data rows
    for row_vals in &all_values {
        let row: String = cols
            .iter()
            .zip(&widths)
            .enumerate()
            .map(|(i, (col, &w))| {
                let val = row_vals.get(i).map_or("", |v| v.as_str());
                match col.align {
                    Align::Left => format!("{:<width$}", val, width = w),
                    Align::Right => format!("{:>width$}", val, width = w),
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        out.push(row);
    }

    out.join("\n")
}

/// A scored entity in a ranked list.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Scored<E: Entity> {
    pub entity: E,
    pub score: f64,
    /// Optional secondary scores for display (e.g. "lines", "tokens").
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub aux: BTreeMap<String, f64>,
}

impl<E: Entity> Scored<E> {
    /// Create a scored entity with no auxiliary scores.
    pub fn new(entity: E, score: f64) -> Self {
        Self {
            entity,
            score,
            aux: BTreeMap::new(),
        }
    }

    /// Create a scored entity with auxiliary scores.
    pub fn with_aux(entity: E, score: f64, aux: BTreeMap<String, f64>) -> Self {
        Self { entity, score, aux }
    }
}

/// Stats computed over a full ranked list before truncation.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RankStats {
    pub total_count: usize,
    pub avg: f64,
    pub max: f64,
    pub min: f64,
}

impl RankStats {
    /// Compute stats from an iterator of scores.
    pub fn from_scores(scores: impl Iterator<Item = f64>) -> Self {
        let mut total_count = 0usize;
        let mut sum = 0.0f64;
        let mut max = f64::NEG_INFINITY;
        let mut min = f64::INFINITY;

        for s in scores {
            total_count += 1;
            sum += s;
            if s > max {
                max = s;
            }
            if s < min {
                min = s;
            }
        }

        if total_count == 0 {
            return Self {
                total_count: 0,
                avg: 0.0,
                max: 0.0,
                min: 0.0,
            };
        }

        Self {
            total_count,
            avg: sum / total_count as f64,
            max,
            min,
        }
    }
}

/// Sort, compute stats, truncate — the shared rank pipeline.
///
/// Sorts `items` by score (descending by default, ascending if `ascending` is true),
/// computes [`RankStats`] over the full list, then truncates to `limit`.
/// A `limit` of 0 means no truncation.
pub fn rank_pipeline<E: Entity>(
    items: &mut Vec<Scored<E>>,
    limit: usize,
    ascending: bool,
) -> RankStats {
    if ascending {
        items.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal));
    } else {
        items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
    }
    let stats = RankStats::from_scores(items.iter().map(|s| s.score));
    if limit > 0 && items.len() > limit {
        items.truncate(limit);
    }
    stats
}

/// Sort by custom comparator, compute stats from a score function, and truncate.
///
/// Like [`rank_pipeline`] but works directly on `Vec<T>` with multi-key sorts
/// where a single `f64` score doesn't capture the full ordering.
/// A `limit` of 0 means no truncation.
pub fn rank_and_truncate<T>(
    items: &mut Vec<T>,
    limit: usize,
    cmp: impl Fn(&T, &T) -> Ordering,
    score: impl Fn(&T) -> f64,
) -> RankStats {
    items.sort_by(|a, b| cmp(a, b));
    let stats = RankStats::from_scores(items.iter().map(&score));
    if limit > 0 && items.len() > limit {
        items.truncate(limit);
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileEntity;

    #[test]
    fn test_rank_pipeline_descending() {
        let mut items = vec![
            Scored::new(
                FileEntity {
                    path: "a.rs".into(),
                },
                3.0,
            ),
            Scored::new(
                FileEntity {
                    path: "b.rs".into(),
                },
                7.0,
            ),
            Scored::new(
                FileEntity {
                    path: "c.rs".into(),
                },
                1.0,
            ),
            Scored::new(
                FileEntity {
                    path: "d.rs".into(),
                },
                5.0,
            ),
        ];
        let stats = rank_pipeline(&mut items, 2, false);

        assert_eq!(stats.total_count, 4);
        assert!((stats.avg - 4.0).abs() < f64::EPSILON);
        assert!((stats.max - 7.0).abs() < f64::EPSILON);
        assert!((stats.min - 1.0).abs() < f64::EPSILON);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].entity.path, "b.rs");
        assert_eq!(items[1].entity.path, "d.rs");
    }

    #[test]
    fn test_rank_pipeline_ascending() {
        let mut items = vec![
            Scored::new(
                FileEntity {
                    path: "a.rs".into(),
                },
                3.0,
            ),
            Scored::new(
                FileEntity {
                    path: "b.rs".into(),
                },
                7.0,
            ),
            Scored::new(
                FileEntity {
                    path: "c.rs".into(),
                },
                1.0,
            ),
        ];
        let stats = rank_pipeline(&mut items, 2, true);

        assert_eq!(stats.total_count, 3);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].entity.path, "c.rs");
        assert_eq!(items[1].entity.path, "a.rs");
    }

    #[test]
    fn test_rank_pipeline_no_limit() {
        let mut items = vec![
            Scored::new(
                FileEntity {
                    path: "a.rs".into(),
                },
                3.0,
            ),
            Scored::new(
                FileEntity {
                    path: "b.rs".into(),
                },
                7.0,
            ),
        ];
        let stats = rank_pipeline(&mut items, 0, false);

        assert_eq!(stats.total_count, 2);
        assert_eq!(items.len(), 2); // no truncation
    }

    #[test]
    fn test_rank_pipeline_empty() {
        let mut items: Vec<Scored<FileEntity>> = Vec::new();
        let stats = rank_pipeline(&mut items, 10, false);

        assert_eq!(stats.total_count, 0);
        assert!((stats.avg - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rank_and_truncate() {
        let mut items = vec![
            ("a.rs", 3usize, 10usize),
            ("b.rs", 1, 20),
            ("c.rs", 3, 5),
            ("d.rs", 2, 15),
        ];
        // Sort by first key ascending, then second key descending
        let stats = rank_and_truncate(
            &mut items,
            3,
            |a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)),
            |item| item.1 as f64,
        );

        assert_eq!(stats.total_count, 4);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].0, "b.rs"); // lowest first key
        assert_eq!(items[1].0, "d.rs"); // second lowest
    }

    #[test]
    fn test_rank_stats_from_scores() {
        let stats = RankStats::from_scores([1.0, 2.0, 3.0, 4.0, 5.0].into_iter());
        assert_eq!(stats.total_count, 5);
        assert!((stats.avg - 3.0).abs() < f64::EPSILON);
        assert!((stats.max - 5.0).abs() < f64::EPSILON);
        assert!((stats.min - 1.0).abs() < f64::EPSILON);
    }

    // ── format_ranked_table tests ──────────────────────────────────────

    #[derive(Clone)]
    struct TestEntry {
        name: String,
        score: usize,
    }

    impl RankEntry for TestEntry {
        fn columns() -> Vec<Column> {
            vec![Column::left("Name"), Column::right("Score")]
        }

        fn values(&self) -> Vec<String> {
            vec![self.name.clone(), self.score.to_string()]
        }
    }

    #[test]
    fn test_format_ranked_table_basic() {
        let entries = vec![
            TestEntry {
                name: "alpha".into(),
                score: 100,
            },
            TestEntry {
                name: "beta".into(),
                score: 42,
            },
        ];
        let text = format_ranked_table("# Test Report", &entries, None);
        assert!(text.contains("# Test Report"));
        assert!(text.contains("Name"));
        assert!(text.contains("Score"));
        assert!(text.contains("alpha"));
        assert!(text.contains("100"));
        assert!(text.contains("beta"));
        assert!(text.contains("42"));
    }

    #[test]
    fn test_format_ranked_table_empty() {
        let entries: Vec<TestEntry> = vec![];
        let text = format_ranked_table("# Empty", &entries, Some("Nothing here."));
        assert!(text.contains("Nothing here."));
        assert!(!text.contains("Name")); // no header for empty
    }

    #[test]
    fn test_format_ranked_table_alignment() {
        let entries = vec![
            TestEntry {
                name: "a".into(),
                score: 1,
            },
            TestEntry {
                name: "long name".into(),
                score: 9999,
            },
        ];
        let text = format_ranked_table("# Align", &entries, None);
        let lines: Vec<&str> = text.lines().collect();
        // Header line should have right-aligned Score header
        let header = lines[2]; // title, blank, header
        assert!(header.contains("Name"));
        assert!(header.contains("Score"));
        // Data: "a" should be left-padded to match "long name" width
        let row_a = lines[4]; // title, blank, header, sep, row
        assert!(row_a.starts_with("a"));
    }
}
