//! Ranked list helpers: scoring, stats, and the shared rank pipeline.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;

use crate::Entity;

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
    fn test_rank_stats_from_scores() {
        let stats = RankStats::from_scores([1.0, 2.0, 3.0, 4.0, 5.0].into_iter());
        assert_eq!(stats.total_count, 5);
        assert!((stats.avg - 3.0).abs() < f64::EPSILON);
        assert!((stats.max - 5.0).abs() < f64::EPSILON);
        assert!((stats.min - 1.0).abs() < f64::EPSILON);
    }
}
