//! Aggregation strategy and computation.

use serde::{Deserialize, Serialize};

/// Aggregation strategy for reducing multiple values to one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Aggregate {
    #[default]
    /// Arithmetic mean of all values.
    Mean,
    /// Middle value (interpolated for even-length inputs).
    Median,
    /// Maximum value.
    Max,
    /// Minimum value.
    Min,
    /// Sum of all values.
    Sum,
    /// Count of items (ignores values, counts occurrences).
    Count,
}

impl std::str::FromStr for Aggregate {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mean" => Ok(Aggregate::Mean),
            "median" => Ok(Aggregate::Median),
            "max" => Ok(Aggregate::Max),
            "min" => Ok(Aggregate::Min),
            "sum" => Ok(Aggregate::Sum),
            "count" => Ok(Aggregate::Count),
            other => Err(format!(
                "unknown aggregation '{other}'; expected mean|median|max|min|sum|count"
            )),
        }
    }
}

impl std::fmt::Display for Aggregate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Aggregate::Mean => "mean",
            Aggregate::Median => "median",
            Aggregate::Max => "max",
            Aggregate::Min => "min",
            Aggregate::Sum => "sum",
            Aggregate::Count => "count",
        };
        f.write_str(s)
    }
}

/// Compute an aggregated value from a list of measurements.
///
/// NaN and infinite values are filtered out before aggregation.
/// Returns `None` if the input is empty or all values are non-finite.
pub fn compute_aggregate(values: Vec<f64>, strategy: Aggregate) -> Option<f64> {
    let mut values: Vec<f64> = values.into_iter().filter(|v| v.is_finite()).collect();
    if values.is_empty() {
        return None;
    }
    Some(match strategy {
        Aggregate::Mean => values.iter().sum::<f64>() / values.len() as f64,
        Aggregate::Median => {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = values.len() / 2;
            if values.len().is_multiple_of(2) {
                (values[mid - 1] + values[mid]) / 2.0
            } else {
                values[mid]
            }
        }
        Aggregate::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        Aggregate::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
        Aggregate::Sum => values.iter().sum(),
        Aggregate::Count => values.len() as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_mean() {
        assert_eq!(
            compute_aggregate(vec![1.0, 2.0, 3.0], Aggregate::Mean),
            Some(2.0)
        );
    }

    #[test]
    fn test_aggregate_median_odd() {
        assert_eq!(
            compute_aggregate(vec![3.0, 1.0, 2.0], Aggregate::Median),
            Some(2.0)
        );
    }

    #[test]
    fn test_aggregate_median_even() {
        assert_eq!(
            compute_aggregate(vec![1.0, 2.0, 3.0, 4.0], Aggregate::Median),
            Some(2.5)
        );
    }

    #[test]
    fn test_aggregate_max() {
        assert_eq!(
            compute_aggregate(vec![1.0, 5.0, 3.0], Aggregate::Max),
            Some(5.0)
        );
    }

    #[test]
    fn test_aggregate_count() {
        assert_eq!(
            compute_aggregate(vec![1.0, 2.0, 3.0], Aggregate::Count),
            Some(3.0)
        );
    }

    #[test]
    fn test_aggregate_empty() {
        assert_eq!(compute_aggregate(vec![], Aggregate::Mean), None);
    }

    #[test]
    fn test_aggregate_nan_filtered() {
        assert_eq!(
            compute_aggregate(vec![1.0, f64::NAN, 3.0], Aggregate::Mean),
            Some(2.0)
        );
    }

    #[test]
    fn test_aggregate_all_nan_returns_none() {
        assert_eq!(
            compute_aggregate(vec![f64::NAN, f64::INFINITY], Aggregate::Mean),
            None
        );
    }
}
